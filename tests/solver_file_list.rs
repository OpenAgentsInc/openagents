use anyhow::Result;
use openagents::solver::file_list::generate_file_list;
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
async fn test_file_list_generation() -> Result<()> {
    let temp_dir = setup_test_repo()?;
    std::env::set_current_dir(&temp_dir)?;

    let repo_map = "src/main.rs:\n\
        fn main()\n\
        \n\
        src/lib.rs:\n\
        fn add(a: i32, b: i32) -> i32";

    let (files, reasoning) = generate_file_list(
        "Add multiply function",
        "Add a multiply function that multiplies two integers",
        repo_map,
        "test_url",
    )
    .await?;

    // Verify file selection
    assert!(!files.is_empty());
    assert!(files.contains(&"src/lib.rs".to_string()));
    assert!(!files.contains(&"src/main.rs".to_string()));

    // Verify reasoning
    assert!(!reasoning.is_empty());
    assert!(reasoning.contains("lib.rs"));
    assert!(reasoning.contains("multiply"));

    Ok(())
}

#[tokio::test]
async fn test_file_list_with_invalid_paths() -> Result<()> {
    let temp_dir = setup_test_repo()?;
    std::env::set_current_dir(&temp_dir)?;

    let repo_map = "src/main.rs:\n\
        fn main()\n\
        \n\
        src/lib.rs:\n\
        fn add(a: i32, b: i32) -> i32\n\
        \n\
        src/nonexistent.rs:\n\
        // This file doesn't exist";

    let (files, _) = generate_file_list(
        "Update all files",
        "Make changes to all files",
        repo_map,
        "test_url",
    )
    .await?;

    // Verify invalid files are filtered out
    assert!(!files.contains(&"src/nonexistent.rs".to_string()));
    assert!(files.iter().all(|path| std::path::Path::new(path).exists()));

    Ok(())
}

#[tokio::test]
async fn test_file_list_empty_repo() -> Result<()> {
    let temp_dir = tempfile::tempdir()?;
    std::env::set_current_dir(&temp_dir)?;

    let (files, reasoning) = generate_file_list(
        "Add new file",
        "Create a new file with some functionality",
        "",
        "test_url",
    )
    .await?;

    assert!(files.is_empty());
    assert!(!reasoning.is_empty());
    assert!(reasoning.contains("No files"));

    Ok(())
}
