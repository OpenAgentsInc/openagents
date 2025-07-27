use crate::error::CommandResult;
use crate::commands::system::{greet, get_project_directory};

#[tokio::test]
async fn test_greet_command() {
    // Test the simple greet command
    let result = greet("Rust Tests");
    assert_eq!(result, "Hello, Rust Tests! You've been greeted from Rust!");
}

#[tokio::test]
async fn test_get_project_directory() {
    // Test getting project directory
    let result = get_project_directory().unwrap();
    
    assert!(result.success);
    assert!(result.data.is_some());
    
    // The directory should be a non-empty string
    let dir = result.data.unwrap();
    assert!(!dir.is_empty());
}

#[tokio::test]
async fn test_command_result_success() {
    // Test CommandResult success construction
    let result: CommandResult<String> = CommandResult::success("test data".to_string());
    
    assert!(result.success);
    assert_eq!(result.data, Some("test data".to_string()));
    assert!(result.error.is_none());
}

#[tokio::test]
async fn test_command_result_error() {
    // Test CommandResult error construction
    let result: CommandResult<String> = CommandResult::error("test error".to_string());
    
    assert!(!result.success);
    assert!(result.data.is_none());
    assert_eq!(result.error, Some("test error".to_string()));
}