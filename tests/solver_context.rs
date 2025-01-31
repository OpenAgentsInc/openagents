use anyhow::Result;
use openagents::solver::context::SolverContext;
use openagents::solver::types::{Change, ChangeError};
use std::fs;
use tempfile::TempDir;

fn setup_test_context() -> Result<(SolverContext, TempDir)> {
    let temp_dir = tempfile::tempdir()?;
    let context = SolverContext::new()?;
    Ok((context, temp_dir))
}

#[test]
fn test_context_initialization() -> Result<()> {
    let (_context, _temp_dir) = setup_test_context()?;
    Ok(())
}

#[test]
fn test_apply_changes_new_file() -> Result<()> {
    let (context, temp_dir) = setup_test_context()?;
    let test_file = temp_dir.path().join("new_function.rs");

    let changes = vec![Change {
        path: test_file.to_str().unwrap().to_string(),
        search: String::new(),
        replace: "fn new_function() {}".to_string(),
        reason: Some("Adding new function".to_string()),
    }];

    context.apply_changes(&changes)?;
    assert!(test_file.exists());
    assert_eq!(fs::read_to_string(&test_file)?, "fn new_function() {}");

    Ok(())
}

#[test]
fn test_apply_changes_modify_file() -> Result<()> {
    let (context, temp_dir) = setup_test_context()?;
    let test_file = temp_dir.path().join("old_function.rs");

    // Create initial file
    fs::write(&test_file, "fn old_function() {}")?;

    let changes = vec![Change {
        path: test_file.to_str().unwrap().to_string(),
        search: "fn old_function() {}".to_string(),
        replace: "fn new_function() {}".to_string(),
        reason: Some("Updating function name".to_string()),
    }];

    context.apply_changes(&changes)?;
    assert_eq!(fs::read_to_string(&test_file)?, "fn new_function() {}");

    Ok(())
}

#[test]
fn test_apply_changes_no_match() -> Result<()> {
    let (context, temp_dir) = setup_test_context()?;
    let test_file = temp_dir.path().join("existing_function.rs");

    // Create initial file
    fs::write(&test_file, "fn existing_function() {}")?;

    let changes = vec![Change {
        path: test_file.to_str().unwrap().to_string(),
        search: "fn non_existent() {}".to_string(),
        replace: "fn new_function() {}".to_string(),
        reason: Some("Updating non-existent function".to_string()),
    }];

    let result = context.apply_changes(&changes);
    assert!(matches!(result, Err(e) if e.downcast_ref::<ChangeError>().is_some()));
    assert_eq!(
        fs::read_to_string(&test_file)?,
        "fn existing_function() {}"
    );

    Ok(())
}

#[test]
fn test_apply_changes_file_not_found() -> Result<()> {
    let (context, temp_dir) = setup_test_context()?;
    let test_file = temp_dir.path().join("non_existent.rs");

    let changes = vec![Change {
        path: test_file.to_str().unwrap().to_string(),
        search: "fn old() {}".to_string(),
        replace: "fn new() {}".to_string(),
        reason: Some("Updating non-existent file".to_string()),
    }];

    let result = context.apply_changes(&changes);
    assert!(matches!(result, Err(e) if e.downcast_ref::<ChangeError>().is_some()));

    Ok(())
}

#[test]
fn test_cleanup() -> Result<()> {
    let (context, temp_dir) = setup_test_context()?;
    let test_file = temp_dir.path().join("test.rs");
    fs::write(&test_file, "test content")?;

    context.cleanup()?;
    assert!(!test_file.exists());

    Ok(())
}