use anyhow::{anyhow, Result};
use serde_json::Value;
use tracing::{debug, error, info};

use crate::solver::json::fix_common_json_issues;

pub struct PlanningContext {
    pub title: String,
    pub description: String,
}

impl PlanningContext {
    pub fn new(title: String, description: String) -> Self {
        Self {
            title,
            description,
        }
    }

    fn validate_llm_response(response: &str) -> Result<bool> {
        // Check if response is valid JSON
        if let Ok(json) = serde_json::from_str::<Value>(response) {
            // Validate required fields
            if let Some(obj) = json.as_object() {
                if obj.contains_key("plan") && obj.contains_key("reasoning") {
                    return Ok(true);
                }
            }
        }
        Ok(false)
    }

    pub async fn generate_plan(&self) -> Result<String> {
        info!("Generating implementation plan...");

        let prompt = format!(
            "You are a software architect. Generate an implementation plan for this issue:\n\n\
            Title: {}\n\
            Description: {}\n\n\
            Provide a detailed plan including:\n\
            1. Overview of changes needed\n\
            2. Files to modify\n\
            3. Implementation steps\n\
            4. Testing strategy\n\
            5. Potential risks\n\n\
            Format your response as a JSON object with 'plan' and 'reasoning' fields.",
            self.title, self.description
        );

        debug!("Sending prompt to LLM:\n{}", prompt);

        // TODO: Replace with actual LLM call
        let response = "Placeholder response";

        // Validate and fix response
        let fixed_response = fix_common_json_issues(response)?;
        if Self::validate_llm_response(&fixed_response)? {
            Ok(fixed_response)
        } else {
            error!("Invalid LLM response format");
            Err(anyhow!("Invalid response format from LLM"))
        }
    }

    pub fn generate_pr_title(&self) -> String {
        format!("Implement solution for {}", self.title)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_llm_response() {
        let valid_response = r#"{
            "plan": "Implementation steps...",
            "reasoning": "Because..."
        }"#;
        assert!(PlanningContext::validate_llm_response(valid_response).unwrap());

        let invalid_response = r#"{
            "something": "else"
        }"#;
        assert!(!PlanningContext::validate_llm_response(invalid_response).unwrap());
    }

    #[test]
    fn test_generate_pr_title() {
        let context = PlanningContext::new(
            "Add login feature".to_string(),
            "Implement user login".to_string(),
        );
        assert_eq!(
            context.generate_pr_title(),
            "Implement solution for Add login feature"
        );
    }
}