use anyhow::Result;
use futures_util::StreamExt;
use openagents::solver::{
    changes::generation::generate_changes,
    file_list::generate_file_list,
    planning::PlanningContext,
};
use std::fs;
use tempfile::TempDir;

fn setup_test_repo() -> Result<TempDir> {
    let temp_dir = tempfile::tempdir()?;

    // Create test files
    fs::create_dir_all(temp_dir.path().join("src"))?;
    fs::write(
        temp_dir.path().join("src/main.rs"),
        "fn main() { println!(\"Hello\"); }",
    )?;
    fs::write(
        temp_dir.path().join("src/lib.rs"),
        "pub fn add(a: i32, b: i32) -> i32 { a + b }",
    )?;

    Ok(temp_dir)
}

#[tokio::test]
#[ignore = "requires local Ollama server"]
async fn test_ollama_file_list() -> Result<()> {
    let temp_dir = setup_test_repo()?;
    std::env::set_current_dir(&temp_dir)?;

    let repo_map = "src/main.rs\nsrc/lib.rs";
    let (files, reasoning) = generate_file_list(
        "Add multiply function",
        "Add a multiply function to lib.rs",
        repo_map,
        "http://localhost:11434",
    )
    .await?;

    assert!(!files.is_empty());
    assert!(files.contains(&"src/lib.rs".to_string()));
    assert!(!files.contains(&"src/main.rs".to_string()));
    assert!(!reasoning.is_empty());
    assert!(reasoning.contains("lib.rs"));

    Ok(())
}

#[tokio::test]
#[ignore = "requires local Ollama server"]
async fn test_ollama_planning() -> Result<()> {
    std::env::set_var("OLLAMA_URL", "http://localhost:11434");
    std::env::set_var("OLLAMA_MODEL", "codellama:latest");

    let context = PlanningContext::new()?;
    let mut stream = context
        .generate_plan(
            123,
            "Add multiply function",
            "Add a multiply function to lib.rs",
            "src/main.rs\nsrc/lib.rs",
        )
        .await?;

    let mut saw_content = false;
    while let Some(result) = stream.next().await {
        match result {
            Ok(content) => {
                println!("Content: {}", content);
                saw_content = true;
            }
            Err(e) => {
                println!("Error: {}", e);
                break;
            }
        }
    }

    assert!(saw_content);
    Ok(())
}

#[tokio::test]
#[ignore = "requires local Ollama server"]
async fn test_ollama_changes() -> Result<()> {
    let temp_dir = setup_test_repo()?;
    std::env::set_current_dir(&temp_dir)?;

    let (changes, reasoning) = generate_changes(
        "src/lib.rs",
        "pub fn add(a: i32, b: i32) -> i32 { a + b }",
        "Add multiply function",
        "Add a multiply function to lib.rs",
        "http://localhost:11434",
    )
    .await?;

    assert!(!changes.is_empty());
    assert_eq!(changes[0].path, "src/lib.rs");
    assert!(changes[0].replace.contains("multiply"));
    assert!(!reasoning.is_empty());

    Ok(())
}