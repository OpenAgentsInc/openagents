use anyhow::Result;
use futures_util::StreamExt;
use openagents::server::services::{gateway::Gateway, ollama::service::OllamaService};

#[tokio::test]
async fn test_ollama_metadata() {
    let service = OllamaService::new();
    let metadata = service.metadata();
    assert_eq!(metadata.name, "ollama");
    assert_eq!(metadata.openai_compatible, false);
    assert!(metadata.supported_features.contains(&"chat".to_string()));
    assert!(metadata
        .supported_features
        .contains(&"streaming".to_string()));
}

#[tokio::test]
#[ignore = "requires local Ollama server"]
async fn test_ollama_chat() -> Result<()> {
    let service = OllamaService::new();
    let (response, _) = service
        .chat("Count from 1 to 5 slowly.".to_string(), false)
        .await?;
    println!("\nChat response:\n{}\n", response);
    assert!(!response.is_empty());
    Ok(())
}

#[tokio::test]
#[ignore = "requires local Ollama server"]
async fn test_ollama_chat_stream() -> Result<()> {
    let service = OllamaService::new();
    println!("\nStreaming response:");
    let mut stream = service
        .chat_stream("Count from 1 to 5 slowly.".to_string(), false)
        .await?;

    let mut saw_content = false;
    while let Some(result) = stream.next().await {
        match result {
            Ok(content) => {
                print!("{}", content);
                std::io::Write::flush(&mut std::io::stdout())?;
                saw_content = true;
            }
            Err(e) => {
                if e.to_string() != "Stream ended" {
                    panic!("Unexpected error: {}", e);
                }
                break;
            }
        }
    }
    println!("\n");
    assert!(saw_content);
    Ok(())
}

#[tokio::test]
#[ignore = "requires local Ollama server"]
async fn test_ollama_with_config() {
    let service = OllamaService::with_config("http://localhost:11434", "deepseek-r1:14b");
    let metadata = service.metadata();
    assert_eq!(metadata.default_model, "deepseek-r1:14b");
    assert!(metadata
        .available_models
        .contains(&"deepseek-r1:14b".to_string()));
}

#[tokio::test]
#[ignore = "requires local Ollama server"]
async fn test_ollama_error_handling() {
    let service = OllamaService::with_config("http://invalid-url", "invalid-model");
    let result = service.chat("Test message".to_string(), false).await;
    assert!(result.is_err());
}
