use anyhow::Result;
use tokio::sync::broadcast;
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::info;
use crate::server::services::{
    solver::ws::types::{SolverStage, SolverUpdate},
    github_types::Issue,
    StreamUpdate,
};

impl super::super::SolverService {
    pub async fn analyze_files(
        &self,
        repomap: &str, 
        issue: &Issue,
        update_tx: broadcast::Sender<SolverUpdate>,
    ) -> Result<(Vec<String>, String)> {
        let files_prompt = format!(
            "Given this GitHub repository map:\n\n{}\n\n\
            And this GitHub issue:\nTitle: {}\nDescription: {}\n\n\
            Based on the repository structure and issue description, analyze which files would be most relevant to review for solving this issue.\n\
            Consider:\n\
            1. Files that would need to be modified\n\
            2. Related files for context\n\
            3. Test files that would need updating\n\
            4. Configuration files if relevant\n\n\
            Format your final answer as a markdown list with one file per line, starting each line with a hyphen (-).",
            repomap,
            issue.title,
            issue.body
        );

        // Create shared state using tokio::sync::Mutex
        let files_state = Arc::new(Mutex::new((String::new(), String::new())));
        let update_tx_clone = update_tx.clone();
        let files_state_clone = files_state.clone();

        // Stream the files analysis
        let mut stream = self.deepseek_service.chat_stream(files_prompt, true).await;

        while let Some(update) = stream.recv().await {
            match update {
                StreamUpdate::Content(content) => {
                    let mut guard = files_state_clone.lock().await;
                    guard.0.push_str(&content);
                    let _ = update_tx_clone.send(SolverUpdate::Progress {
                        stage: SolverStage::Analysis,
                        message: "Analyzing files...".into(),
                        data: Some(serde_json::json!({
                            "files_list": guard.0,
                            "files_reasoning": guard.1
                        })),
                    });
                }
                StreamUpdate::Reasoning(reasoning) => {
                    let mut guard = files_state_clone.lock().await;
                    guard.1.push_str(&reasoning);
                    let _ = update_tx_clone.send(SolverUpdate::Progress {
                        stage: SolverStage::Analysis,
                        message: "Analyzing files...".into(),
                        data: Some(serde_json::json!({
                            "files_list": guard.0,
                            "files_reasoning": guard.1
                        })),
                    });
                }
                StreamUpdate::Done => break,
            }
        }

        // Get final results
        let state = files_state.lock().await;
        let files_list = state.0.clone();
        let files_reasoning = state.1.clone();
        drop(state);

        info!("Files response: {}", files_list);
        info!("Files reasoning: {}", files_reasoning);

        // Parse the response as a markdown list
        let files: Vec<String> = files_list
            .lines()
            .filter(|line| line.trim().starts_with("- "))
            .map(|line| line.trim().trim_start_matches("- ").trim().to_string())
            .collect();

        info!("Parsed files: {:?}", files);

        // Send solution progress update with reasoning
        let _ = update_tx.send(SolverUpdate::Progress {
            stage: SolverStage::Solution,
            message: "Analyzing solution approach".into(),
            data: Some(serde_json::json!({
                "files": files.clone(),
                "reasoning": files_reasoning.clone()
            })),
        });

        Ok((files, files_reasoning))
    }
}
