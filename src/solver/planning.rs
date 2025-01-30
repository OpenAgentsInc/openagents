use anyhow::{anyhow, Result};
use serde_json::json;
use tracing::{debug, error, info};

pub struct PlanningContext {
    pub llm_service: crate::server::services::deepseek::DeepSeekService,
}

impl PlanningContext {
    pub fn new(ollama_url: &str) -> Result<Self> {
        Ok(Self {
            llm_service: crate::server::services::deepseek::DeepSeekService::new(ollama_url.to_string()),
        })
    }

    pub async fn generate_plan(
        &self,
        issue_number: i32,
        title: &str,
        description: &str,
        repo_map: &str,
        file_context: &str,
    ) -> Result<String> {
        let prompt = format!(
            r#"Generate a plan for issue #{} based on:
Title: {}
Description: {}
Repository Map: {}
File Context: {}"#,
            issue_number, title, description, repo_map, file_context
        );

        let (response, _) = self.llm_service.chat(prompt, true).await?;
        Ok(response)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mockito::Server;

    #[tokio::test]
    async fn test_validate_llm_response() {
        let mut server = Server::new();
        let mock_response = json!({
            "choices": [{
                "message": {
                    "content": "feat: add multiply function"
                }
            }]
        });

        let mock = server.mock("POST", "/v1/chat/completions")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(mock_response.to_string())
            .create();

        std::env::set_var("DEEPSEEK_API_URL", &server.url());
        
        let context = PlanningContext::new("test_url").unwrap();
        let result = context.generate_plan(
            123,
            "Add multiply function",
            "Add a multiply function",
            "src/main.rs",
            "test context",
        ).await;

        mock.assert();
        assert!(result.is_ok());
    }
}