use anyhow::Result;
use futures_util::StreamExt;
use std::pin::Pin;
use termcolor::Color;
use tracing::info;

pub async fn handle_plan_stream(
    mut stream: Pin<Box<dyn futures_util::Stream<Item = Result<String>> + Send>>,
) -> Result<String> {
    let mut full_response = String::new();

    while let Some(result) = stream.next().await {
        match result {
            Ok(content) => {
                full_response.push_str(&content);
                crate::solver::display::print_colored(&content, Color::White)?;
            }
            Err(e) => {
                info!("Error in stream: {}", e);
                break;
            }
        }
    }

    crate::solver::display::print_colored(
        "\nPlan generation complete.\n",
        Color::Green,
    )?;

    Ok(full_response)
}