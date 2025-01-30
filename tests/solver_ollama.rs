use anyhow::Result;
use openagents::solver::{
    changes::generation::generate_changes,
    file_list::generate_file_list,
};
use std::fs;
use tempfile::tempdir;
use mockito::Server;
use serde_json::json;

#[tokio::test]
async fn test_ollama_file_list() -> Result<()> {
    let temp_dir = tempdir()?;
    let test_file = temp_dir.path().join("test.rs");
    fs::write(&test_file, "// Original content")?;

    let mut server = Server::new();
    let mock_response = json!({
        "choices": [{
            "message": {
                "content": "src/lib.rs"
            }
        }]
    });

    let mock = server.mock("POST", "/v1/chat/completions")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(mock_response.to_string())
        .create();

    std::env::set_var("DEEPSEEK_API_URL", &server.url());

    let (files, reasoning) = generate_file_list(
        "Add multiply function",
        "Add a multiply function to lib.rs",
        "src/main.rs\nsrc/lib.rs",
        "test_url",
    )
    .await?;

    mock.assert();
    assert!(!files.is_empty());
    assert!(!reasoning.is_empty());
    Ok(())
}

#[tokio::test]
async fn test_ollama_changes() -> Result<()> {
    let temp_dir = tempdir()?;
    let test_file = temp_dir.path().join("test.rs");
    fs::write(&test_file, "// Original content")?;

    let mut server = Server::new();
    let mock_response = json!({
        "choices": [{
            "message": {
                "content": "New content"
            }
        }]
    });

    let mock = server.mock("POST", "/v1/chat/completions")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(mock_response.to_string())
        .create();

    std::env::set_var("DEEPSEEK_API_URL", &server.url());

    let (changes, reasoning) = generate_changes(
        "test.rs",
        "// Original content",
        "Add multiply function",
        "Add a multiply function",
        "test_url",
    )
    .await?;

    mock.assert();
    assert!(!changes.is_empty());
    assert!(!reasoning.is_empty());
    Ok(())
}