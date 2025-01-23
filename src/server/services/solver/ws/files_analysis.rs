use crate::server::services::{
    deepseek::StreamUpdate,
    solver::{
        ws::{send_message, Message},
        SolverService,
    },
};
use futures::StreamExt;
use std::sync::Arc;
use tokio::sync::mpsc::Sender;
use tracing::error;

pub async fn handle_files_analysis(
    solver: Arc<SolverService>,
    tx: Sender<Message>,
    files_content: String,
) {
    let mut stream = solver.analyze_files(files_content).await;

    let mut current_content = String::new();
    let mut current_reasoning = String::new();

    while let Some(update) = stream.recv().await {
        match update {
            StreamUpdate::Content(text) => {
                current_content.push_str(&text);
                if let Err(e) = send_message(
                    &tx,
                    "content",
                    &current_content,
                    Some(&current_reasoning),
                    false,
                )
                .await
                {
                    error!("Failed to send content update: {}", e);
                }
            }
            StreamUpdate::Reasoning(text) => {
                current_reasoning.push_str(&text);
                if let Err(e) = send_message(
                    &tx,
                    "content",
                    &current_content,
                    Some(&current_reasoning),
                    false,
                )
                .await
                {
                    error!("Failed to send reasoning update: {}", e);
                }
            }
            StreamUpdate::Done => break,
            StreamUpdate::ToolCalls(_) => {
                // Ignore tool calls in files analysis
            }
        }
    }

    // Send final message
    if let Err(e) = send_message(
        &tx,
        "content",
        &current_content,
        Some(&current_reasoning),
        true,
    )
    .await
    {
        error!("Failed to send final message: {}", e);
    }
}