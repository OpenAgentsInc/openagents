use crate::server::services::StreamUpdate;
use anyhow::Result;
use reqwest::Client;
use tokio::sync::mpsc;

pub async fn chat_stream(
    client: &Client,
    prompt: String,
    use_reasoner: bool,
) -> mpsc::Receiver<StreamUpdate> {
    let (tx, rx) = mpsc::channel(100);
    let client = client.clone();

    tokio::spawn(async move {
        match chat_internal(&client, prompt, use_reasoner).await {
            Ok(content) => {
                if use_reasoner {
                    let _ = tx.send(StreamUpdate::ReasoningContent(content.clone())).await;
                }
                let _ = tx.send(StreamUpdate::Content(content)).await;
                let _ = tx.send(StreamUpdate::Done).await;
            }
            Err(e) => {
                let _ = tx.send(StreamUpdate::Error(e.to_string())).await;
            }
        }
    });

    rx
}

async fn chat_internal(client: &Client, prompt: String, _use_reasoner: bool) -> Result<String> {
    // For now, just return the prompt as the response
    Ok(prompt)
}