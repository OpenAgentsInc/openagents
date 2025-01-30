use crate::server::services::deepseek::StreamUpdate;
use anyhow::Result;
use termcolor::Color;
use tokio::sync::mpsc;
use tracing::info;

pub async fn handle_plan_stream(mut stream: mpsc::Receiver<StreamUpdate>) -> Result<String> {
    let mut full_response = String::new();

    while let Some(update) = stream.recv().await {
        match update {
            StreamUpdate::Content(content) => {
                full_response.push_str(&content);
                crate::solver::display::print_colored(&content, Color::White)?;
            }
            StreamUpdate::Reasoning(reasoning) => {
                info!("Planning reasoning: {}", reasoning);
                crate::solver::display::print_colored(
                    &format!("\nReasoning: {}\n", reasoning),
                    Color::Yellow,
                )?;
            }
            StreamUpdate::Done => {
                crate::solver::display::print_colored(
                    "\nPlan generation complete.\n",
                    Color::Green,
                )?;
                break;
            }
            _ => {}
        }
    }

    Ok(full_response)
}