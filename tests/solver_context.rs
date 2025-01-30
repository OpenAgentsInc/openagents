use anyhow::Result;
use openagents::solver::context::SolverContext;
use openagents::solver::{Change, ChangeError};
use std::fs;
use tempfile::TempDir;

fn setup_test_context() -> Result<(SolverContext, TempDir)> {
    let temp_dir = tempfile::tempdir()?;
    let context = SolverContext::new_with_dir(temp_dir.path().to_path_buf());
    Ok((context, temp_dir))
}

#[test]
fn test_context_initialization() -> Result<()> {
    let (context, _temp_dir) = setup_test_context()?;
    assert!(context.temp_dir.exists());
    Ok(())
}

#[test]
fn test_apply_changes_new_file() -> Result<()> {
    let (context, _temp_dir) = setup_test_context()?;

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
    assert!(context.temp_dir.join("src/new_file.rs").exists());

    Ok(())
}

#[test]
fn test_apply_changes_modify_file() -> Result<()> {
    let (context, _temp_dir) = setup_test_context()?;

    // Create initial file
    let file_path = "src/test.rs";
    fs::create_dir_all(context.temp_dir.join("src"))?;
    fs::write(context.temp_dir.join(file_path), "fn old_function() {}")?;

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
    assert!(context.temp_dir.join(file_path).exists());

    Ok(())
}

#[test]
fn test_apply_changes_no_match() -> Result<()> {
    let (context, _temp_dir) = setup_test_context()?;

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
    assert!(!context.temp_dir.join("src/test.rs").exists());

    Ok(())
}

#[test]
fn test_apply_changes_file_not_found() -> Result<()> {
    let (context, _temp_dir) = setup_test_context()?;

    let changes = vec![Change::new(
        "non_existent.rs".to_string(),
        "fn old() {}".to_string(),
        "fn new() {}".to_string(),
    )];

    let result = context.apply_changes(&changes);
    assert!(matches!(result, Err(ChangeError::FileNotFound(_))));
    assert!(!context.temp_dir.join("non_existent.rs").exists());

    Ok(())
}

#[test]
fn test_cleanup() -> Result<()> {
    let temp_dir = tempfile::tempdir()?;
    let temp_path = temp_dir.path().to_path_buf();

    {
        let context = SolverContext::new_with_dir(temp_path.clone());

        // Create some test files
        fs::create_dir_all(context.temp_dir.join("src"))?;
        fs::write(context.temp_dir.join("src/test.rs"), "fn test() {}")?;

        // Verify files exist
        assert!(context.temp_dir.join("src/test.rs").exists());

        // Clean up
        let _ = context.cleanup();
    }

    // Verify directory was cleaned up
    assert!(!temp_path.exists());

    Ok(())
}
