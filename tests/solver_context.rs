use anyhow::Result;
use openagents::solver::{Change, ChangeError, SolutionContext};
use std::fs;
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

#[test]
fn test_context_initialization() -> Result<()> {
    let (context, _temp_dir) = setup_test_context()?;
    assert!(context.temp_dir.exists());
    assert!(context.modified_files.is_empty());
    Ok(())
}

#[test]
fn test_apply_changes_new_file() -> Result<()> {
    let (mut context, _temp_dir) = setup_test_context()?;

    // Create a new file
    let changes = vec![Change::new(
        "src/new_file.rs".to_string(),
        "".to_string(),
        "fn new_function() {}".to_string(),
    )];

    context.apply_changes(&changes)?;

    // Verify file was created
    let file_path = context.temp_dir.join("src/new_file.rs");
    assert!(file_path.exists());
    assert_eq!(fs::read_to_string(file_path)?, "fn new_function() {}");
    assert!(context.modified_files.contains(&"src/new_file.rs".to_string()));

    Ok(())
}

#[test]
fn test_apply_changes_modify_file() -> Result<()> {
    let (mut context, _temp_dir) = setup_test_context()?;

    // Create initial file
    let file_path = "src/test.rs";
    fs::create_dir_all(context.temp_dir.join("src"))?;
    fs::write(
        context.temp_dir.join(file_path),
        "fn old_function() {}",
    )?;

    // Modify the file
    let changes = vec![Change::new(
        file_path.to_string(),
        "fn old_function() {}".to_string(),
        "fn new_function() {}".to_string(),
    )];

    context.apply_changes(&changes)?;

    // Verify file was modified
    assert_eq!(
        fs::read_to_string(context.temp_dir.join(file_path))?,
        "fn new_function() {}"
    );
    assert!(context.modified_files.contains(&file_path.to_string()));

    Ok(())
}

#[test]
fn test_apply_changes_no_match() -> Result<()> {
    let (mut context, _temp_dir) = setup_test_context()?;

    // Create initial file
    let file_path = "src/test.rs";
    fs::create_dir_all(context.temp_dir.join("src"))?;
    fs::write(
        context.temp_dir.join(file_path),
        "fn existing_function() {}",
    )?;

    // Try to modify non-existent content
    let changes = vec![Change::new(
        file_path.to_string(),
        "fn non_existent() {}".to_string(),
        "fn new_function() {}".to_string(),
    )];

    let result = context.apply_changes(&changes);
    assert!(matches!(result, Err(ChangeError::NoMatch)));
    assert!(context.modified_files.is_empty());

    Ok(())
}

#[test]
fn test_apply_changes_file_not_found() -> Result<()> {
    let (mut context, _temp_dir) = setup_test_context()?;

    let changes = vec![Change::new(
        "non_existent.rs".to_string(),
        "fn old() {}".to_string(),
        "fn new() {}".to_string(),
    )];

    let result = context.apply_changes(&changes);
    assert!(matches!(result, Err(ChangeError::FileNotFound(_))));
    assert!(context.modified_files.is_empty());

    Ok(())
}

#[test]
fn test_cleanup() -> Result<()> {
    let temp_dir = tempfile::tempdir()?;
    let temp_path = temp_dir.path().to_path_buf();
    
    {
        let context = SolutionContext::new_with_dir(
            temp_path.clone(),
            "test_key".to_string(),
            Some("test_token".to_string()),
        )?;

        // Create some test files
        fs::create_dir_all(context.temp_dir.join("src"))?;
        fs::write(
            context.temp_dir.join("src/test.rs"),
            "fn test() {}",
        )?;

        // Verify files exist
        assert!(context.temp_dir.join("src/test.rs").exists());

        // Clean up
        context.cleanup();
    }

    // Verify directory was cleaned up
    assert!(!temp_path.exists());

    Ok(())
}