use crate::server::services::deepseek::StreamUpdate;
use crate::server::services::gateway::Gateway;
use crate::server::services::openrouter::{OpenRouterConfig, OpenRouterService};
use anyhow::Result;
use futures::StreamExt;
use tokio::sync::mpsc;
use tracing::debug;

pub struct PlanningContext {
    service: OpenRouterService,
}

impl PlanningContext {
    pub fn new() -> Result<Self> {
        let api_key = std::env::var("OPENROUTER_API_KEY")
            .map_err(|_| anyhow::anyhow!("OPENROUTER_API_KEY not set"))?;

        let config = OpenRouterConfig {
            model: "deepseek/deepseek-chat".to_string(),
            //model: "anthropic/claude-3.5-sonnet".to_string(),
            use_reasoner: true,
            test_mode: false,
        };

        Ok(Self {
            service: OpenRouterService::with_config(api_key, config),
        })
    }

    pub async fn generate_plan(
        &self,
        issue_number: i32,
        title: &str,
        description: &str,
        repo_map: &str,
    ) -> mpsc::Receiver<StreamUpdate> {
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

        debug!("Starting streaming plan generation with OpenRouter");
        let (tx, rx) = mpsc::channel(100);

        // Convert OpenRouter stream to our StreamUpdate format
        let stream_result = self.service.chat_stream(prompt, true).await;

        tokio::spawn(async move {
            match stream_result {
                Ok(mut stream) => {
                    while let Some(result) = stream.next().await {
                        match result {
                            Ok(content) => {
                                let _ = tx.send(StreamUpdate::Content(content)).await;
                            }
                            Err(e) => {
                                debug!("Error in stream: {}", e);
                                break;
                            }
                        }
                    }
                    let _ = tx.send(StreamUpdate::Done).await;
                }
                Err(e) => {
                    debug!("Failed to create stream: {}", e);
                    let _ = tx.send(StreamUpdate::Done).await;
                }
            }
        });

        rx
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

        debug!("Sending planning prompt to OpenRouter");
        let (response, _) = self.service.chat(prompt, true).await?;
        Ok(response)
    }
}
