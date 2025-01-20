use anyhow::Result;
use tokio::sync::{broadcast, Mutex};
use std::sync::Arc;
use crate::server::services::{
    solver_ws::{SolverStage, SolverUpdate},
    github::Issue,
};

impl super::super::SolverService {
    pub(crate) async fn generate_solution(
        &self,
        repomap: &str,
        files: &[String],
        issue: &Issue,
        update_tx: broadcast::Sender<SolverUpdate>,
    ) -> Result<(String, String)> {
        let solution_prompt = format!(
            "Given this GitHub repository map:\n\n{}\n\n\
             And these relevant files:\n{}\n\n\
             For this GitHub issue:\nTitle: {}\nDescription: {}\n\n\
             Analyze and provide a detailed solution including:\n\
             1. Specific code changes needed (with file paths)\n\
             2. Any new files that need to be created\n\
             3. Step-by-step implementation instructions\n\
             4. Potential risks or considerations\n\
             Format the response in markdown with code blocks for any code changes.",
            repomap,
            files.join("\n"),
            issue.title,
            issue.body
        );

        // Create shared state using tokio::sync::Mutex
        let solution_state = Arc::new(Mutex::new((String::new(), String::new())));
        let update_tx_clone = update_tx.clone();
        let solution_state_clone = solution_state.clone();

        // Stream the solution generation
        self.deepseek_service
            .chat_stream(solution_prompt, true, move |content, reasoning| async move {
                let state = solution_state_clone.clone();
                let tx = update_tx_clone.clone();
                let fut = async move {
                    let mut guard = state.lock().await;
                    if let Some(c) = content {
                        guard.0.push_str(c);
                        let _ = tx.send(SolverUpdate::Progress {
                            stage: SolverStage::Solution,
                            message: "Generating solution...".into(),
                            data: Some(serde_json::json!({
                                "solution": guard.0,
                                "reasoning": guard.1
                            })),
                        });
                    }
                    if let Some(r) = reasoning {
                        guard.1.push_str(r);
                        let _ = tx.send(SolverUpdate::Progress {
                            stage: SolverStage::Solution,
                            message: "Generating solution...".into(),
                            data: Some(serde_json::json!({
                                "solution": guard.0,
                                "reasoning": guard.1
                            })),
                        });
                    }
                    Ok(())
                };
                fut.await
            })
            .await?;

        // Get final results
        let state = solution_state.lock().await;
        let solution_text = state.0.clone();
        let solution_reasoning = state.1.clone();
        drop(state);

        // Send PR progress update with reasoning
        let _ = update_tx.send(SolverUpdate::Progress {
            stage: SolverStage::PR,
            message: "Preparing solution".into(),
            data: Some(serde_json::json!({
                "solution": solution_text.clone(),
                "reasoning": solution_reasoning.clone()
            })),
        });

        Ok((solution_text, solution_reasoning))
    }
}
