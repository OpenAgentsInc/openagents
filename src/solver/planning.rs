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
        // Check response focuses on pull request title generation
        if !response.contains("pull request title") || !response.contains("src/solver/github.rs") {
            debug!(
                "Response doesn't focus on pull request title generation or target correct file"
            );
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

        // Verify changes target the correct file and function
        let changes = json["changes"].as_array().unwrap();
        let targets_github_rs = changes.iter().any(|c| {
            c["path"]
                .as_str()
                .unwrap_or("")
                .contains("src/solver/github.rs")
                && c["search"]
                    .as_str()
                    .unwrap_or("")
                    .contains("generate_pr_title")
        });

        if !targets_github_rs {
            debug!("Changes don't target src/solver/github.rs generate_pr_title function");
            return Ok(false);
        }

        Ok(true)
    }

    fn generate_prompt(
        &self,
        issue_number: i32,
        title: &str,
        description: &str,
        repo_map: &str,
        file_context: &str,
        feedback: Option<&str>,
    ) -> String {
        let base_prompt = format!(
            r#"Your task is to improve pull request (PR) title generation in the OpenAgents solver.

Context:
- A pull request is a GitHub feature for submitting code changes
- The solver automatically creates pull requests to solve issues
- PR titles are currently generic like "Implement solution for #{}"
- We want descriptive titles that follow the format "<type>: <description>"

Relevant Files:
{}

Required Changes:
1. Modify ONLY the generate_pr_title function in src/solver/github.rs
2. Use the DeepSeek LLM (self.llm_service.chat) to generate better titles
3. Ensure titles follow the format "<type>: <description>"
   - type must be: feat, fix, refactor, docs, style, test, or perf
   - description must be clear and concise
4. Do not modify test files or other code

Example Good PR Titles:
- "feat: add multiply function to calculator"
- "fix: handle JSON escaping in PR titles"
- "refactor: improve error handling in solver"
- "style: format code with rustfmt"

Example Bad PR Titles:
- "Implement solution for #634" (wrong format)
- "feat:add function" (missing space)
- "update code" (missing type prefix)
- "feat: implement the new feature that was requested in the issue" (too verbose)

Issue Title: {}
Description:
{}

Repository Map:
{}

Output your solution as a JSON object in a markdown code block:

```json
{{
    "changes": [
        {{
            "path": "src/solver/github.rs",
            "search": "exact content to find",
            "replace": "new content",
            "reason": "why this change improves PR title generation"
        }}
    ],
    "reasoning": "Overall explanation of changes"
}}
```

Requirements:
1. Changes MUST focus on pull request title generation
2. Changes MUST target src/solver/github.rs only
3. All strings MUST be properly escaped for JSON
4. PR titles MUST start with "feat:", "fix:", etc.
5. PR titles MUST be descriptive but succinct
6. Do NOT modify test files or other code

Rules:
- Use EXACT content matches for search
- Include enough context for unique matches
- Keep changes minimal and focused
- Preserve code style and formatting
- All strings must be properly escaped
- Use the existing code structure and variables"#,
            issue_number,
            file_context,
            escape_json_string(title),
            escape_json_string(description),
            escape_json_string(repo_map)
        );

        if let Some(feedback) = feedback {
            format!(
                "{}\n\nPrevious attempt feedback:\n{}",
                base_prompt, feedback
            )
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
        file_context: &str,
    ) -> Result<String> {
        let mut feedback = None;

        for attempt in 0..MAX_RETRIES {
            let prompt = self.generate_prompt(
                issue_number,
                title,
                description,
                repo_map,
                file_context,
                feedback,
            );
            let (response, _) = self.service.chat(prompt, true).await?;

            // Try to fix common JSON issues
            let fixed_response = fix_common_json_issues(&response);

            if Self::validate_llm_response(&fixed_response)? {
                return Ok(fixed_response);
            }

            // Generate feedback for next attempt
            feedback = Some(
                "Previous attempt was invalid because:\n\
                - Response must focus on pull request title generation\n\
                - Must modify src/solver/github.rs generate_pr_title function\n\
                - Must use existing code structure and variables\n\
                - All strings must be properly escaped\n\
                - PR titles must start with feat:, fix:, etc.\n\
                - Do not modify test files",
            );

            info!("Attempt {} failed, retrying with feedback", attempt + 1);
        }

        Err(anyhow!(
            "Failed to generate valid response after {} attempts",
            MAX_RETRIES
        ))
    }

    pub async fn generate_plan(
        &self,
        issue_number: i32,
        title: &str,
        description: &str,
        repo_map: &str,
        file_context: &str,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<String>> + Send>>> {
        let prompt = self.generate_prompt(
            issue_number,
            title,
            description,
            repo_map,
            file_context,
            None,
        );
        self.service.chat_stream(prompt, true).await
    }

    pub async fn generate_plan_sync(
        &self,
        issue_number: i32,
        title: &str,
        description: &str,
        repo_map: &str,
        file_context: &str,
    ) -> Result<String> {
        self.retry_with_feedback(issue_number, title, description, repo_map, file_context)
            .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[ignore = "requires LLM setup"]
    fn test_validate_llm_response() {
        // Valid response
        let valid = r#"{
            "changes": [{
                "path": "src/solver/github.rs",
                "search": "async fn generate_pr_title",
                "replace": "async fn generate_pr_title(&self, issue_number: i32, context: &str) -> Result<String> {
                    let prompt = format!(\"Generate a descriptive pull request title that starts with feat:, fix:, etc.\");
                    let (response, _) = self.llm_service.chat(prompt, true).await?;
                    Ok(response.trim().to_string())
                }",
                "reason": "Improve pull request title generation"
            }],
            "reasoning": "Improve pull request title generation with better prompting"
        }"#;
        assert!(PlanningContext::validate_llm_response(valid).unwrap());

        // Invalid - no changes
        let no_changes = r#"{
            "changes": [],
            "reasoning": "No changes needed"
        }"#;
        assert!(!PlanningContext::validate_llm_response(no_changes).unwrap());

        // Invalid - wrong file
        let wrong_file = r#"{
            "changes": [{
                "path": "src/test.rs",
                "search": "old",
                "replace": "new",
                "reason": "Update test"
            }],
            "reasoning": "Update tests"
        }"#;
        assert!(!PlanningContext::validate_llm_response(wrong_file).unwrap());

        // Invalid - no pull request focus
        let no_pr = r#"{
            "changes": [{
                "path": "src/solver/github.rs",
                "search": "fn test_something",
                "replace": "fn test_something_new",
                "reason": "Update test"
            }],
            "reasoning": "Update tests"
        }"#;
        assert!(!PlanningContext::validate_llm_response(no_pr).unwrap());
    }

    #[test]
    fn test_generate_prompt() {
        let context = PlanningContext::new("test_url").unwrap();
        let prompt = context.generate_prompt(
            123,
            "Test title",
            "Test description",
            "Test repo map",
            "Test file context",
            None,
        );

        // Check for key phrases that indicate proper context
        assert!(prompt.contains("pull request"));
        assert!(prompt.contains("src/solver/github.rs"));
        assert!(prompt.contains("generate_pr_title"));
        assert!(prompt.contains("feat:"));
        assert!(prompt.contains("must be properly escaped"));

        // Check for examples
        assert!(prompt.contains("feat: add multiply function"));
        assert!(prompt.contains("Implement solution for #"));

        // Check for clear instructions
        assert!(prompt.contains("Do NOT modify test files"));
        assert!(prompt.contains("MUST target src/solver/github.rs"));
        assert!(prompt.contains("Use the existing code structure"));
    }
}
