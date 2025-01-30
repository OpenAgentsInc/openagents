use anyhow::Result;
use openagents::solver::{
    changes::generation::generate_changes,
    file_list::generate_file_list,
    planning::PlanningContext,
};
use std::fs;
use tempfile::tempdir;

#[tokio::test]
async fn test_ollama_file_list() -> Result<()> {
    let temp_dir = tempdir()?;
    let test_file = temp_dir.path().join("test.rs");
    fs::write(&test_file, "// Original content")?;

    let (files, reasoning) = generate_file_list(
        "Add multiply function",
        "Add a multiply function to lib.rs",
        "src/main.rs\nsrc/lib.rs",
        "test_url",
    )
    .await?;

    assert!(!files.is_empty());
    assert!(!reasoning.is_empty());
    Ok(())
}

#[tokio::test]
async fn test_ollama_planning() -> Result<()> {
    let temp_dir = tempdir()?;
    let test_file = temp_dir.path().join("test.rs");
    fs::write(&test_file, "// Original content")?;

    let context = PlanningContext::new("test_url")?;
    let result = context
        .generate_plan(
            123,
            "Add multiply function",
            "Add a multiply function to lib.rs",
            "src/main.rs\nsrc/lib.rs",
            "test_context",
        )
        .await;

    assert!(result.is_ok());
    Ok(())
}

#[tokio::test]
async fn test_ollama_changes() -> Result<()> {
    let temp_dir = tempdir()?;
    let test_file = temp_dir.path().join("test.rs");
    fs::write(&test_file, "// Original content")?;

    let (changes, reasoning) = generate_changes(
        "test.rs",
        "// Original content",
        "Add multiply function",
        "Add a multiply function",
        "test_url",
    )
    .await?;

    assert!(!changes.is_empty());
    assert!(!reasoning.is_empty());
    Ok(())
}