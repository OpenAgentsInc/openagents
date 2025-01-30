use anyhow::{anyhow, Result};
use serde_json::json;
use tracing::{debug, error};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_llm_response() {
        let json_str = r#"{
            "changes": [{
                "path": "src/solver/github.rs",
                "search": "generate_pr_title",
                "replace": "let (response, _) = self.llm_service.chat(prompt, true).await?;",
                "reason": "Improve PR title generation"
            }],
            "reasoning": "Enhanced PR title generation"
        }"#;

        let result = validate_llm_response(json_str, "Improve PR title generation");
        assert!(result.is_ok());
        assert!(result.unwrap());
    }
}