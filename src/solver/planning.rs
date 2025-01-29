use crate::server::services::gateway::Gateway;
use crate::server::services::openrouter::OpenRouterService;
use anyhow::Result;

pub struct PlanningContext {
    gateway: Box<dyn Gateway + Send + Sync>,
}

impl PlanningContext {
    pub fn new() -> Result<Self> {
        let api_key = std::env::var("OPENROUTER_API_KEY")
            .map_err(|_| anyhow::anyhow!("OPENROUTER_API_KEY not found in environment"))?;

        let gateway = Box::new(OpenRouterService::new(api_key)?);

        Ok(Self { gateway })
    }

    pub async fn generate_plan(
        &self,
        issue_number: i32,
        issue_title: &str,
        issue_body: &str,
        repo_map: &str,
    ) -> Result<String> {
        let prompt = format!(
            r#"You are an expert software developer tasked with implementing a solution for a GitHub issue.
Based on the issue details and repository structure, create an implementation plan.

Issue #{}: {}
Description:
{}

Repository Structure:
{}

Create a detailed implementation plan that includes:
1. Files that need to be modified
2. Key functionality to implement
3. Dependencies and requirements
4. Testing strategy

Format your response in Markdown."#,
            issue_number, issue_title, issue_body, repo_map
        );

        let (response, _) = self.gateway.chat(prompt, true).await?;
        Ok(response)
    }
}