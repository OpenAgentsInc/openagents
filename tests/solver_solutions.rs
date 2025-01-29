use anyhow::Result;
use openagents::solver::{Change, SolutionContext};
use std::fs;
use std::path::PathBuf;
use tempfile::TempDir;

fn setup_test_context() -> Result<(SolutionContext, TempDir)> {
    let temp_dir = tempfile::tempdir()?;
    let context = SolutionContext::new_with_dir(
        temp_dir.path().to_path_buf(),
        "test_key".to_string(),
        Some("test_token".to_string()),
    )?;
    Ok((context, temp_dir))
}

#[tokio::test]
async fn test_generate_file_list() -> Result<()> {
    let (context, _temp_dir) = setup_test_context()?;

    // Create test files
    fs::write(
        context.temp_dir.join("src/main.rs"),
        "fn main() { println!(\"Hello\"); }",
    )?;
    fs::write(
        context.temp_dir.join("src/lib.rs"),
        "pub fn add(a: i32, b: i32) -> i32 { a + b }",
    )?;

    let files = context
        .generate_file_list(
            "Add multiply function",
            "Add a multiply function to lib.rs",
            "src/main.rs\nsrc/lib.rs",
        )
        .await?;

    assert_eq!(files.len(), 1);
    assert_eq!(files[0], "src/lib.rs");

    Ok(())
}

#[tokio::test]
async fn test_generate_changes() -> Result<()> {
    let (context, _temp_dir) = setup_test_context()?;

    let file_content = "pub fn add(a: i32, b: i32) -> i32 { a + b }";
    let changes = context
        .generate_changes(
            "src/lib.rs",
            file_content,
            "Add multiply function",
            "Add a multiply function that multiplies two integers",
        )
        .await?;

    assert!(!changes.is_empty());
    assert_eq!(changes[0].path, "src/lib.rs");
    assert!(changes[0].replace.contains("multiply"));

    Ok(())
}

#[tokio::test]
async fn test_apply_changes() -> Result<()> {
    let (mut context, _temp_dir) = setup_test_context()?;

    // Create test file
    let file_path = "src/lib.rs";
    let original_content = "pub fn add(a: i32, b: i32) -> i32 { a + b }";
    fs::create_dir_all(context.temp_dir.join("src"))?;
    fs::write(context.temp_dir.join(file_path), original_content)?;

    // Create test change
    let changes = vec![Change {
        path: file_path.to_string(),
        search: original_content.to_string(),
        replace: format!("{}\n\npub fn multiply(a: i32, b: i32) -> i32 {{ a * b }}", original_content),
    }];

    // Apply changes
    context.apply_changes(&changes)?;

    // Verify file was modified
    let modified_content = fs::read_to_string(context.temp_dir.join(file_path))?;
    assert!(modified_content.contains("multiply"));
    assert!(modified_content.contains("add"));
    assert!(context.modified_files.contains(&file_path.to_string()));

    Ok(())
}

#[tokio::test]
async fn test_full_solution_flow() -> Result<()> {
    let (mut context, _temp_dir) = setup_test_context()?;

    // Setup test repository
    fs::create_dir_all(context.temp_dir.join("src"))?;
    fs::write(
        context.temp_dir.join("src/lib.rs"),
        "pub fn add(a: i32, b: i32) -> i32 { a + b }",
    )?;

    // Generate file list
    let files = context
        .generate_file_list(
            "Add multiply function",
            "Add a multiply function to lib.rs",
            "src/lib.rs",
        )
        .await?;

    assert_eq!(files.len(), 1);
    assert_eq!(files[0], "src/lib.rs");

    // For each file, generate and apply changes
    for file_path in files {
        let current_content = fs::read_to_string(context.temp_dir.join(&file_path))?;
        
        let changes = context
            .generate_changes(
                &file_path,
                &current_content,
                "Add multiply function",
                "Add a multiply function that multiplies two integers",
            )
            .await?;

        context.apply_changes(&changes)?;
    }

    // Verify results
    assert!(!context.modified_files.is_empty());
    let final_content = fs::read_to_string(context.temp_dir.join("src/lib.rs"))?;
    assert!(final_content.contains("multiply"));
    assert!(final_content.contains("add"));

    Ok(())
}