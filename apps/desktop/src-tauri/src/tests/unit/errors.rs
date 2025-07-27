use crate::claude_code::models::ClaudeError;
use std::io;

#[test]
fn test_claude_error_from_io_error() {
    // Test all IO error kinds conversion
    let io_errors = vec![
        (io::ErrorKind::NotFound, "file not found"),
        (io::ErrorKind::PermissionDenied, "permission denied"),
        (io::ErrorKind::ConnectionRefused, "connection refused"),
        (io::ErrorKind::BrokenPipe, "broken pipe"),
        (io::ErrorKind::AlreadyExists, "already exists"),
        (io::ErrorKind::InvalidData, "invalid data"),
    ];
    
    for (kind, message) in io_errors {
        let io_error = io::Error::new(kind, message);
        let claude_error = ClaudeError::from(io_error);
        
        match claude_error {
            ClaudeError::IoError(err) => {
                assert_eq!(err.kind(), kind);
                assert_eq!(err.to_string(), message);
            }
            _ => panic!("Expected IoError variant"),
        }
    }
}

#[test]
fn test_claude_error_from_serde_json_error() {
    // Test JSON parsing error conversion
    let invalid_json = "{ invalid json }";
    let result: Result<serde_json::Value, _> = serde_json::from_str(invalid_json);
    
    assert!(result.is_err());
    let json_error = result.unwrap_err();
    let claude_error = ClaudeError::from(json_error);
    
    match claude_error {
        ClaudeError::JsonError(_) => {
            // Expected error type
        }
        _ => panic!("Expected JsonError variant"),
    }
}

#[test]
fn test_claude_error_display() {
    // Test error message formatting
    let errors = vec![
        (ClaudeError::BinaryNotFound, "Claude Code binary not found"),
        (ClaudeError::SessionNotFound("test-123".to_string()), "Session not found: test-123"),
    ];
    
    for (error, expected_message) in errors {
        assert_eq!(error.to_string(), expected_message);
    }
    
    // Test IO error display
    let io_error = ClaudeError::from(io::Error::new(io::ErrorKind::NotFound, "file not found"));
    assert!(io_error.to_string().contains("I/O error"));
    
    // Test JSON error display
    let json_err: Result<serde_json::Value, _> = serde_json::from_str("invalid");
    if let Err(e) = json_err {
        let claude_error = ClaudeError::from(e);
        assert!(claude_error.to_string().contains("JSON parsing error"));
    }
}

#[test]
fn test_claude_error_chaining() {
    // Test error chain preservation
    let io_error = io::Error::new(io::ErrorKind::NotFound, "config.json");
    let claude_error = ClaudeError::from(io_error);
    
    // Verify we can still access the underlying error
    match &claude_error {
        ClaudeError::IoError(err) => {
            assert_eq!(err.kind(), io::ErrorKind::NotFound);
            assert!(err.to_string().contains("config.json"));
        }
        _ => panic!("Expected IoError"),
    }
}

#[test]
fn test_error_recovery_scenarios() {
    // Test various error recovery patterns
    fn handle_error(error: ClaudeError) -> String {
        match error {
            ClaudeError::BinaryNotFound => "Please install Claude CLI".to_string(),
            ClaudeError::SessionNotFound(_) => "Session expired, please restart".to_string(),
            ClaudeError::IoError(_) => "File system error occurred".to_string(),
            ClaudeError::JsonError(_) => "Invalid response format".to_string(),
            ClaudeError::HttpError(_) => "Network request failed".to_string(),
            ClaudeError::Other(msg) => format!("Error: {}", msg),
        }
    }
    
    assert_eq!(handle_error(ClaudeError::BinaryNotFound), "Please install Claude CLI");
    assert_eq!(handle_error(ClaudeError::SessionNotFound("123".to_string())), "Session expired, please restart");
    
    // Test IO error handling
    let io_error = ClaudeError::from(io::Error::new(io::ErrorKind::PermissionDenied, "denied"));
    assert_eq!(handle_error(io_error), "File system error occurred");
}

#[test]
fn test_error_serialization() {
    // Test that errors can be serialized for logging/debugging
    let error = ClaudeError::SessionNotFound("test-session".to_string());
    let error_string = format!("{:?}", error);
    
    assert!(error_string.contains("SessionNotFound"));
    assert!(error_string.contains("test-session"));
    
    // Test other error types
    let binary_error = ClaudeError::BinaryNotFound;
    let binary_string = format!("{:?}", binary_error);
    assert!(binary_string.contains("BinaryNotFound"));
}

#[test]
fn test_result_type_conversions() {
    // Test Result<T, ClaudeError> patterns
    fn might_fail(should_fail: bool) -> Result<String, ClaudeError> {
        if should_fail {
            Err(ClaudeError::SessionNotFound("test-session".to_string()))
        } else {
            Ok("success".to_string())
        }
    }
    
    // Test success case
    let result = might_fail(false);
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), "success");
    
    // Test failure case
    let result = might_fail(true);
    assert!(result.is_err());
    match result {
        Err(ClaudeError::SessionNotFound(id)) => assert_eq!(id, "test-session"),
        _ => panic!("Unexpected result"),
    }
}

#[test]
fn test_error_propagation() {
    // Test error propagation through multiple layers
    fn inner_function() -> Result<(), io::Error> {
        Err(io::Error::new(io::ErrorKind::PermissionDenied, "access denied"))
    }
    
    fn middle_function() -> Result<(), ClaudeError> {
        inner_function()?;
        Ok(())
    }
    
    let result = middle_function();
    assert!(result.is_err());
    
    match result {
        Err(ClaudeError::IoError(e)) => {
            assert_eq!(e.kind(), io::ErrorKind::PermissionDenied);
        }
        _ => panic!("Expected IoError"),
    }
}

#[test]
fn test_custom_error_contexts() {
    // Test adding context to errors
    fn read_config(path: &str) -> Result<String, ClaudeError> {
        std::fs::read_to_string(path)
            .map_err(|e| ClaudeError::from(e))
    }
    
    let result = read_config("/nonexistent/config.json");
    assert!(result.is_err());
    
    // Verify error contains useful context
    if let Err(ClaudeError::IoError(e)) = result {
        assert_eq!(e.kind(), io::ErrorKind::NotFound);
    }
}