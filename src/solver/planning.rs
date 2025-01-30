use crate::server::services::gateway::Gateway;
use anyhow::Result;
use futures_util::{Stream, StreamExt};
use std::pin::Pin;

pub struct PlanningContext {
    service: crate::server::services::ollama::OllamaService,
}

impl PlanningContext {
    pub fn new() -> Result<Self> {
        Ok(Self {
            service: crate::server::services::ollama::OllamaService::new(),
        })
    }

    pub async fn generate_plan(
        &self,
        issue_number: i32,
        title: &str,
        description: &str,
        repo_map: &str,
    ) -> Result<Pin<Box<dyn Stream<Item = Result<String>> + Send>>> {
        let prompt = format!(
            r#"You are an expert software developer. Your task is to analyze this GitHub issue and generate an implementation plan.

Issue #{}: {}

Description:
{}

Repository Map:
{}

Output a detailed implementation plan that:
1. Lists files that need to be modified
2. Describes specific changes needed
3. Explains implementation steps
4. Provides rationale for changes

Focus on minimal, precise changes that directly address the issue requirements.
"#,
            issue_number, title, description, repo_map
        );

        self.service.chat_stream(prompt, true).await
    }

    // Keep the old method for backwards compatibility during transition
    pub async fn generate_plan_sync(
        &self,
        issue_number: i32,
        title: &str,
        description: &str,
        repo_map: &str,
    ) -> Result<String> {
        let prompt = format!(
            r#"You are an expert software developer. Your task is to analyze this GitHub issue and generate an implementation plan.

Issue #{}: {}

Description:
{}

Repository Map:
{}

Output a detailed implementation plan that:
1. Lists files that need to be modified
2. Describes specific changes needed
3. Explains implementation steps
4. Provides rationale for changes

Focus on minimal, precise changes that directly address the issue requirements.
"#,
            issue_number, title, description, repo_map
        );

        let (response, _) = self.service.chat(prompt, true).await?;
        Ok(response)
    }
}