use anyhow::Result;
use tokio::sync::broadcast;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::info;
use crate::server::services::{
    solver::ws::types::{SolverStage, SolverUpdate},
    github_types::Issue,
};

impl super::super::SolverService {
    pub async fn generate_solution(
        &self,
        repomap: &str,
        files: &[String],
        issue: &Issue,
        update_tx: broadcast::Sender<SolverUpdate>,
    ) -> Result<(String, String)> {
        let solution_prompt = format!(
            "Given this GitHub repository map:\n\n{}\n\n\
            And this GitHub issue:\nTitle: {}\nDescription: {}\n\n\
            For these relevant files:\n{}\n\n\
            Generate a detailed solution for this issue. Consider:\n\
            1. Required code changes\n\
            2. Test updates needed\n\
            3. Configuration changes\n\
            4. Migration steps if needed\n\n\
            Format your solution in markdown with clear sections and code blocks.",
            repomap,
            issue.title,
            issue.body,
            files.join("\n")
        );

        // Create shared state using tokio::sync::Mutex
        let solution_state = Arc::new(Mutex::new((String::new(), String::new())));
        let update_tx_clone = update_tx.clone();
        let solution_state_clone = solution_state.clone();

        // Stream the solution generation
        self.deepseek_service
            .chat_stream(solution_prompt, true, move |content, reasoning| {
                let state = solution_state_clone.clone();
                let tx = update_tx_clone.clone();
                
                tokio::spawn(async move {
                    let mut guard = state.lock().await;
                    if let Some(c) = content {
                        guard.0.push_str(c);
                        let _ = tx.send(SolverUpdate::Progress {
                            stage: SolverStage::Solution,
                            message: "Generating solution...".into(),
                            data: Some(serde_json::json!({
                                "solution_text": guard.0,
                                "solution_reasoning": guard.1
                            })),
                        });
                    }
                    if let Some(r) = reasoning {
                        guard.1.push_str(r);
                        let _ = tx.send(SolverUpdate::Progress {
                            stage: SolverStage::Solution,
                            message: "Generating solution...".into(),
                            data: Some(serde_json::json!({
                                "solution_text": guard.0,
                                "solution_reasoning": guard.1
                            })),
                        });
                    }
                });
                Ok(())
            })
            .await?;

        // Get final results
        let state = solution_state.lock().await;
        let solution_text = state.0.clone();
        let solution_reasoning = state.1.clone();
        drop(state);

        info!("Solution text: {}", solution_text);
        info!("Solution reasoning: {}", solution_reasoning);

        // Send PR stage update
        let _ = update_tx.send(SolverUpdate::Progress {
            stage: SolverStage::PR,
            message: "Preparing pull request".into(),
            data: Some(serde_json::json!({
                "solution": solution_text.clone(),
                "reasoning": solution_reasoning.clone()
            })),
        });

        Ok((solution_text, solution_reasoning))
    }
}