use anyhow::Result;
use futures_util::StreamExt;
use openagents::solver::{
    changes::generation::generate_changes,
    file_list::generate_file_list,
    planning::PlanningContext,
};
use serial_test::serial;
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

fn load_env() {
    dotenvy::dotenv().ok();
    // Set defaults if not in .env
    if std::env::var("OLLAMA_URL").is_err() {
        std::env::set_var("OLLAMA_URL", "http://localhost:11434");
    }
    if std::env::var("OLLAMA_MODEL").is_err() {
        std::env::set_var("OLLAMA_MODEL", "codellama:latest");
    }
}

#[tokio::test]
#[serial]
#[ignore = "requires local Ollama server"]
async fn test_ollama_file_list() -> Result<()> {
    load_env();
    let temp_dir = setup_test_repo()?;
    std::env::set_current_dir(&temp_dir)?;

    let repo_map = "src/main.rs\nsrc/lib.rs";
    let ollama_url = std::env::var("OLLAMA_URL").unwrap();
    
    let (files, reasoning) = generate_file_list(
        "Add multiply function",
        "Add a multiply function to lib.rs",
        repo_map,
        &ollama_url,
    )
    .await?;

    println!("\nFiles selected:");
    for file in &files {
        println!("- {}", file);
    }
    println!("\nReasoning:\n{}", reasoning);

    assert!(!files.is_empty());
    assert!(files.contains(&"src/lib.rs".to_string()));
    assert!(!files.contains(&"src/main.rs".to_string()));
    assert!(!reasoning.is_empty());
    assert!(reasoning.contains("lib.rs"));

    Ok(())
}

#[tokio::test]
#[serial]
#[ignore = "requires local Ollama server"]
async fn test_ollama_planning() -> Result<()> {
    load_env();
    let context = PlanningContext::new()?;
    
    println!("\nStreaming plan:\n");
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
                print!("{}", content);
                std::io::Write::flush(&mut std::io::stdout())?;
                saw_content = true;
            }
            Err(e) => {
                println!("\nError: {}", e);
                break;
            }
        }
    }
    println!("\n"); // Add final newlines

    assert!(saw_content);
    Ok(())
}

#[tokio::test]
#[serial]
#[ignore = "requires local Ollama server"]
async fn test_ollama_changes() -> Result<()> {
    load_env();
    let temp_dir = setup_test_repo()?;
    std::env::set_current_dir(&temp_dir)?;

    let ollama_url = std::env::var("OLLAMA_URL").unwrap();

    println!("\nGenerating changes...\n");
    let (changes, reasoning) = generate_changes(
        "src/lib.rs",
        "pub fn add(a: i32, b: i32) -> i32 { a + b }",
        "Add multiply function",
        "Add a multiply function to lib.rs",
        &ollama_url,
    )
    .await?;

    println!("Changes:");
    for change in &changes {
        println!("\nFile: {}", change.path);
        println!("Replace with:\n{}", change.replace);
    }
    println!("\nReasoning:\n{}", reasoning);

    assert!(!changes.is_empty());
    assert_eq!(changes[0].path, "src/lib.rs");
    assert!(changes[0].replace.contains("multiply"));
    assert!(!reasoning.is_empty());

    Ok(())
}