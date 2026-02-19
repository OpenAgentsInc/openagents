//! Input validation for issue data
//!
//! This module provides validation functions for all user-provided issue data to ensure
//! data quality, prevent invalid states, and enforce business rules before data enters
//! the database.
//!
//! # Architecture
//!
//! The validation module follows a fail-fast approach:
//! - Each field is validated independently
//! - Validation fails immediately on the first error
//! - Detailed error types help callers understand what went wrong
//! - All validation is pure and side-effect free
//!
//! # Design Decisions
//!
//! - **Early Validation**: All data is validated before database insertion to prevent
//!   invalid states from ever existing in the system
//! - **Explicit Limits**: Hardcoded length limits prevent resource exhaustion and
//!   ensure consistent UX across all interfaces
//! - **No Trimming**: Validation rejects leading/trailing whitespace rather than
//!   silently trimming to avoid surprising users
//! - **Case-Sensitive Agents**: Agent names must be exact lowercase matches to
//!   prevent confusion and ensure consistency
//!
//! # Usage
//!
//! ```
//! use issues::validation::{validate_title, validate_description, validate_agent};
//!
//! // Validate all fields before creating an issue
//! let title = validate_title("Fix authentication bug")?;
//! let description = validate_description(Some("Details about the bug"))?;
//! let agent = validate_agent("codex")?;
//!
//! // Now safe to insert into database
//! # Ok::<(), issues::validation::ValidationError>(())
//! ```
//!
//! # Testing
//!
//! This module includes both traditional unit tests and property-based tests using
//! proptest to verify validation logic across a wide range of inputs. See the tests
//! module for comprehensive examples of all error cases.

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

    #[error("Agent name must be 'codex' or 'codex' (got '{0}')")]
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
///
/// # Examples
///
/// ```
/// use issues::validation::{validate_title, ValidationError};
///
/// // Valid title
/// assert!(validate_title("Fix authentication bug").is_ok());
///
/// // Empty title (error)
/// assert_eq!(validate_title(""), Err(ValidationError::TitleEmpty));
///
/// // Whitespace-only title (error)
/// assert_eq!(validate_title("   "), Err(ValidationError::TitleEmpty));
///
/// // Title too long (error)
/// let long_title = "a".repeat(201);
/// assert!(matches!(validate_title(&long_title), Err(ValidationError::TitleTooLong(201))));
///
/// // Leading whitespace (error)
/// assert_eq!(
///     validate_title("  Fix bug"),
///     Err(ValidationError::TitleHasWhitespace)
/// );
///
/// // Trailing whitespace (error)
/// assert_eq!(
///     validate_title("Fix bug  "),
///     Err(ValidationError::TitleHasWhitespace)
/// );
///
/// // Unicode is valid
/// assert!(validate_title("修复认证错误").is_ok());
/// ```
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
///
/// # Examples
///
/// ```
/// use issues::validation::{validate_description, ValidationError};
///
/// // None is valid
/// assert_eq!(validate_description(None), Ok(None));
///
/// // Valid description
/// assert!(validate_description(Some("This is a test description")).is_ok());
///
/// // Description with newlines is valid
/// assert!(validate_description(Some("Line 1\nLine 2\nLine 3")).is_ok());
///
/// // Description too long (error)
/// let long_desc = "a".repeat(10_001);
/// assert!(matches!(
///     validate_description(Some(&long_desc)),
///     Err(ValidationError::DescriptionTooLong(10_001))
/// ));
/// ```
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
/// - Must be "codex" or "codex"
///
/// # Examples
///
/// ```
/// use issues::validation::{validate_agent, ValidationError};
///
/// // Valid agents
/// assert_eq!(validate_agent("codex"), Ok("codex".to_string()));
///
/// // Invalid agent names (error)
/// assert_eq!(
///     validate_agent("gpt4"),
///     Err(ValidationError::InvalidAgent("gpt4".to_string()))
/// );
///
/// // Empty string (error)
/// assert_eq!(
///     validate_agent(""),
///     Err(ValidationError::InvalidAgent("".to_string()))
/// );
///
/// // Case-sensitive (error)
/// assert_eq!(
///     validate_agent("Codex"),
///     Err(ValidationError::InvalidAgent("Codex".to_string()))
/// );
/// ```
pub fn validate_agent(agent: &str) -> Result<String, ValidationError> {
    match agent {
        "codex" => Ok(agent.to_string()),
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
        assert_eq!(result, Err(ValidationError::InvalidAgent("".to_string())));
    }

    #[test]
    fn test_invalid_agent_case_sensitive() {
        // Agent names are case-sensitive
        let result = validate_agent("Codex");
        assert_eq!(
            result,
            Err(ValidationError::InvalidAgent("Codex".to_string()))
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
        fn prop_title_rejects_too_long(s in "[a-z]{201,220}") {
            // Reduced range from 201-300 to 201-220 for faster test execution
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
        fn prop_description_accepts_valid_length(s in "[a-zA-Z0-9 \\n]{0,500}") {
            // Reduced max length from 10000 to 500 for faster test execution
            // while still testing the core validation logic
            let result = validate_description(Some(&s));
            prop_assert!(result.is_ok());
            prop_assert_eq!(result.unwrap(), Some(s));
        }

        #[test]
        fn prop_description_rejects_too_long(s in "[a-z]{10001,10020}") {
            // Reduced range from 10001-10100 to 10001-10020 for faster test execution
            let result = validate_description(Some(&s));
            prop_assert!(result.is_err());
            prop_assert!(matches!(result, Err(ValidationError::DescriptionTooLong(_))));
        }

        #[test]
        fn prop_agent_accepts_only_valid_names(s in "[a-z]{1,20}") {
            let result = validate_agent(&s);
            if s == "codex" || s == "codex" {
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
