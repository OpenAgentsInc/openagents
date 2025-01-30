use anyhow::Result;
use futures_util::StreamExt;
use std::{io::Write, pin::Pin};
use tracing::{debug, info};

pub async fn handle_plan_stream(
    mut stream: Pin<Box<dyn futures_util::Stream<Item = Result<String>> + Send>>,
) -> Result<String> {
    let mut full_response = String::new();
    debug!("Starting to handle plan stream...");

    while let Some(result) = stream.next().await {
        match result {
            Ok(content) => {
                debug!("Received content: {}", content);
                full_response.push_str(&content);
                print!("{}", content);
                std::io::stdout().flush()?;
            }
            Err(e) => {
                info!("Error in stream: {}", e);
                break;
            }
        }
    }

    if full_response.is_empty() {
        info!("Warning: No content received from stream");
    } else {
        debug!("Full response: {}", full_response);
    }

    println!("\nPlan generation complete.\n");

    Ok(full_response)
}
