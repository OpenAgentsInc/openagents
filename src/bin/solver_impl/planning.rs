use anyhow::Result;
use openagents::solver::{
    planning::PlanningContext,
    streaming::handle_plan_stream,
};
use tracing::info;

pub async fn handle_planning(
    issue_number: i32,
    title: &str,
    description: &str,
    repo_map: &str,
    ollama_url: &str,
) -> Result<String> {
    info!("Generating implementation plan...");

    let context = PlanningContext::new(ollama_url)?;
    let stream = context
        .generate_plan(issue_number, title, description, repo_map)
        .await?;

    handle_plan_stream(stream).await
}