use crate::server::services::gateway::Gateway;
use crate::solver::json::{escape_json_string, fix_common_json_issues, is_valid_json_string};
use anyhow::{anyhow, Result};
use futures_util::Stream;
use serde_json::Value;
use std::pin::Pin;
use tracing::{debug, info};

const MAX_RETRIES: u32 = 3;

pub struct PlanningContext {
    service: crate::server::services::ollama::OllamaService,
}

impl PlanningContext {
    pub fn new(ollama_url: &str) -> Result<Self> {
        Ok(Self {
            service: crate::server::services::ollama::OllamaService::with_config(
                ollama_url,
                "deepseek-r1:14b",
            ),
        })
    }

    fn validate_llm_response(response: &str) -> Result<bool> {
        // Check response focuses on PR title
        if !response.contains("PR title") || !response.contains("feat:") {
            debug!("Response doesn't focus on PR title or use feat: prefix");
            return Ok(false);
        }
        
        // Validate JSON structure
        let json = serde_json::from_str::<Value>(response)?;
        if json["changes"].as_array().map_or(0, |a| a.len()) == 0 {
            debug!("Response contains no changes");
            return Ok(false);
        }
        
        // Check for proper string escaping
        for change in json["changes"].as_array().unwrap() {
            let search = change["search"].as_str().unwrap_or("");
            let replace = change["replace"].as_str().unwrap_or("");
            if !is_valid_json_string(search) || !is_valid_json_string(replace) {
                debug!("Invalid JSON string escaping in change block");
                return Ok(false);
            }
        }
        
        Ok(true)
    }

    fn generate_prompt(
        &self,
        issue_number: i32,
        title: &str,
        description: &str,
        repo_map: &str,
        feedback: Option<&str>,
    ) -> String {
        let base_prompt = format!(
            r#"Your task is to improve PR title generation in the solver.

Current behavior: PR titles are like "Implement solution for #{}"
Desired behavior: PR titles should be descriptive and succinct
Example: "feat: add multiply function" instead of "Implement solution for #634"

Issue Title: {}
Description:
{}

Repository Map:
{}

Focus ONLY on the PR title generation logic. Output your solution as a JSON object in a markdown code block:

```json
{{
    "changes": [
        {{
            "path": "path/to/file",
            "search": "exact content to find",
            "replace": "new content",
            "reason": "why this change is needed"
        }}
    ],
    "reasoning": "Overall explanation of changes"
}}
```

Requirements:
1. Changes MUST focus on PR title generation
2. All strings MUST be properly escaped for JSON
3. PR titles MUST start with "feat:", "fix:", etc.
4. PR titles MUST be descriptive but succinct
5. Changes should modify the title generation code, not test files

Rules:
- Use EXACT content matches for search
- Include enough context for unique matches
- Keep changes minimal and focused
- Preserve code style and formatting
- All strings must be properly escaped"#,
            issue_number,
            escape_json_string(title),
            escape_json_string(description),
            escape_json_string(repo_map)
        );

        if let Some(feedback) = feedback {
            format!("{}\n\nPrevious attempt feedback:\n{}", base_prompt, feedback)
        } else {
            base_prompt
        }
    }

    async fn retry_with_feedback(
        &self,
        issue_number: i32,
        title: &str,
        description: &str,
        repo_map: &str,
    ) -> Result<String> {
        let mut feedback = None;
        
        for attempt in 0..MAX_RETRIES {
            let prompt = self.generate_prompt(issue_number, title, description, repo_map, feedback);
            let (response, _) = self.service.chat(prompt, true).await?;
            
            // Try to fix common JSON issues
            let fixed_response = fix_common_json_issues(&response);
            
            if Self::validate_llm_response(&fixed_response)? {
                return Ok(fixed_response);
            }
            
            // Generate feedback for next attempt
            feedback = Some(
                "Previous attempt was invalid because:\n\
                - Response must focus on PR title generation\n\
                - Must include actual changes to implement\n\
                - All strings must be properly escaped\n\
                - PR titles must start with feat:, fix:, etc."
            );
            
            info!("Attempt {} failed, retrying with feedback", attempt + 1);
        }
        
        Err(anyhow!("Failed to generate valid response after {} attempts", MAX_RETRIES))
    }

    pub async fn generate_plan(
        &self,
        issue_number: i32,
        title: &str,
        description: &str,
        repo_map: &str,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<String>> + Send>>> {
        let prompt = self.generate_prompt(issue_number, title, description, repo_map, None);
        self.service.chat_stream(prompt, true).await
    }

    pub async fn generate_plan_sync(
        &self,
        issue_number: i32,
        title: &str,
        description: &str,
        repo_map: &str,
    ) -> Result<String> {
        self.retry_with_feedback(issue_number, title, description, repo_map).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_llm_response() {
        // Valid response
        let valid = r#"{
            "changes": [{
                "path": "src/github.rs",
                "search": "fn generate_title",
                "replace": "fn generate_title_with_prefix",
                "reason": "Add feat: prefix to PR titles"
            }],
            "reasoning": "Improve PR title with feat: prefix"
        }"#;
        assert!(PlanningContext::validate_llm_response(valid).unwrap());

        // Invalid - no changes
        let no_changes = r#"{
            "changes": [],
            "reasoning": "No changes needed"
        }"#;
        assert!(!PlanningContext::validate_llm_response(no_changes).unwrap());

        // Invalid - no PR title focus
        let no_title = r#"{
            "changes": [{
                "path": "src/test.rs",
                "search": "old",
                "replace": "new",
                "reason": "Update test"
            }],
            "reasoning": "Update tests"
        }"#;
        assert!(!PlanningContext::validate_llm_response(no_title).unwrap());
    }

    #[test]
    fn test_generate_prompt() {
        let context = PlanningContext::new("test_url").unwrap();
        let prompt = context.generate_prompt(
            123,
            "Test title",
            "Test description",
            "Test repo map",
            None,
        );
        
        assert!(prompt.contains("feat:"));
        assert!(prompt.contains("PR title"));
        assert!(prompt.contains("must be properly escaped"));
    }
}