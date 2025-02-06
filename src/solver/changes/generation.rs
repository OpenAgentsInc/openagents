use crate::solver::json::fix_common_json_issues;
use crate::solver::types::Change;
use crate::solver::changes::types::ChangeResponse;
use anyhow::{anyhow, Result};
use tracing::{debug, error, info};

pub async fn generate_changes(
    title: &str,
    description: &str,
    response: &str,
) -> Result<Vec<Change>> {
    info!("Generating changes from LLM response...");
    debug!("Raw response: {}", response);

    // First try to parse as-is
    if let Ok(change_response) = serde_json::from_str::<ChangeResponse>(response) {
        return process_change_response(change_response, title, description);
    }

    // Try to fix common JSON issues
    let fixed_response = fix_common_json_issues(response)?;
    if fixed_response != response {
        debug!("Fixed JSON formatting issues");
        if let Ok(change_response) = serde_json::from_str::<ChangeResponse>(&fixed_response) {
            return process_change_response(change_response, title, description);
        }
    }

    error!("Failed to parse LLM response as valid JSON");
    Err(anyhow!("Invalid JSON response from LLM"))
}

pub fn validate_changes_relevance(
    changes: &[Change],
    reasoning: &str,
    title: &str,
    description: &str,
) -> bool {
    let keywords = extract_keywords(title, description);
    debug!("Keywords: {:?}", keywords);

    let reasoning_matches = keywords
        .iter()
        .any(|k| reasoning.to_lowercase().contains(&k.to_lowercase()));

    let changes_match = changes.iter().any(|c| {
        keywords
            .iter()
            .any(|k| c.reason.as_ref().is_some_and(|r| r.contains(k)))
    });

    reasoning_matches || changes_match
}

pub fn extract_keywords(title: &str, description: &str) -> Vec<String> {
    let mut keywords = Vec::new();
    let text = format!("{} {}", title, description).to_lowercase();

    // Split on whitespace and punctuation
    for word in text.split(|c: char| c.is_whitespace() || c.is_ascii_punctuation()) {
        let word = word.trim();
        if !word.is_empty() && !is_common_word(word) {
            keywords.push(word.to_string());
        }
    }

    keywords
}

fn is_common_word(word: &str) -> bool {
    let common_words = [
        "the", "be", "to", "of", "and", "a", "in", "that", "have", "i", "it", "for", "not", "on",
        "with", "he", "as", "you", "do", "at", "this", "but", "his", "by", "from", "they", "we",
        "say", "her", "she", "or", "an", "will", "my", "one", "all", "would", "there", "their",
        "what", "so", "up", "out", "if", "about", "who", "get", "which", "go", "me",
    ];
    common_words.contains(&word)
}

fn process_change_response(
    response: ChangeResponse,
    _title: &str,
    _description: &str,
) -> Result<Vec<Change>> {
    let mut changes = Vec::new();

    for block in response.changes {
        let change = Change {
            path: block.path,
            search: block.search,
            replace: block.replace,
            reason: Some(block.reason),
            analysis: String::new(),
        };
        changes.push(change);
    }

    if changes.is_empty() {
        return Err(anyhow!("No valid changes found in response"));
    }

    Ok(changes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_changes() {
        let title = "Fix login bug";
        let description = "The login functionality is broken";
        let response = r#"{
            "changes": [
                {
                    "path": "src/auth.rs",
                    "search": "broken_login()",
                    "replace": "fixed_login()",
                    "reason": "Fix broken login function"
                }
            ]
        }"#;

        let changes = tokio_test::block_on(generate_changes(title, description, response)).unwrap();
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].path, "src/auth.rs");
    }

    #[test]
    fn test_validate_changes_relevance() {
        let change = Change {
            path: "test.rs".to_string(),
            search: "old".to_string(),
            replace: "new".to_string(),
            reason: Some("Fix login bug".to_string()),
            analysis: String::new(),
        };

        let relevant = validate_changes_relevance(
            &[change],
            "Fixing the login functionality",
            "Fix login bug",
            "The login is broken",
        );
        assert!(relevant);
    }

    #[test]
    fn test_extract_keywords() {
        let keywords = extract_keywords(
            "Fix login bug",
            "The login functionality is broken",
        );
        assert!(keywords.contains(&"login".to_string()));
        assert!(keywords.contains(&"bug".to_string()));
        assert!(keywords.contains(&"broken".to_string()));
        assert!(!keywords.contains(&"the".to_string()));
    }
}