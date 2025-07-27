//! Tests for the refactored module structure

#[test]
fn test_error_module_accessible() {
    // Test that error types are accessible
    use crate::error::CommandResult;
    
    let error_result: CommandResult<String> = CommandResult::error("Test error".to_string());
    assert!(!error_result.success);
    assert_eq!(error_result.error, Some("Test error".to_string()));
    
    let success_result: CommandResult<String> = CommandResult::success("Test data".to_string());
    assert!(success_result.success);
    assert_eq!(success_result.data, Some("Test data".to_string()));
}

#[test]
fn test_apm_module_structure() {
    // Test that APM modules are accessible
    use crate::apm::APMStats;
    use crate::apm::models::ProductivityByTime;
    use crate::apm::utils::{calculate_apm, get_tool_category};
    
    // Test basic functions
    assert_eq!(calculate_apm(10, 5, 30.0), 0.5);
    assert_eq!(get_tool_category("Edit"), "Code Generation");
    
    // Test that types can be instantiated
    let _ = ProductivityByTime::default();
    let _ = APMStats::default();
}

#[test]
fn test_commands_module_accessible() {
    // Test that command modules are accessible
    use crate::commands::system::greet;
    
    let greeting = greet("Module Test");
    assert_eq!(greeting, "Hello, Module Test! You've been greeted from Rust!");
}

#[test]
fn test_state_module_accessible() {
    // Test that state module is accessible
    use crate::state::AppState;
    
    let state = AppState::new();
    assert!(state.discovery.try_lock().is_ok());
    assert!(state.manager.try_lock().is_ok());
}

#[test]
fn test_module_imports_in_lib() {
    // This test verifies that all necessary imports work correctly in lib.rs
    // The fact that the application compiles means this is working, but let's be explicit
    use crate::commands::{
        session::discover_claude,
        apm::analyze_claude_conversations,
        history::get_history,
        system::get_project_directory,
    };
    
    // Just verify these symbols exist - we don't need to call them
    let _ = discover_claude as fn(_) -> _;
    let _ = analyze_claude_conversations as fn() -> _;
    let _ = get_history as fn(_, _) -> _;
    let _ = get_project_directory as fn() -> _;
}

#[test]
fn test_apm_analyzer_accessible() {
    // Test that APMAnalyzer is accessible
    use crate::apm::APMAnalyzer;
    
    let analyzer = APMAnalyzer::new();
    // Just verify it can be created
    let _ = analyzer;
}

#[test]
fn test_apm_async_functions_exist() {
    // Test that async APM functions are accessible by importing them
    // The fact that this compiles proves the functions exist
    use crate::apm::{generate_historical_apm_data, fetch_convex_apm_stats};
    
    // Just verify we can reference them
    let _ = generate_historical_apm_data;
    let _ = fetch_convex_apm_stats;
}

#[test]
fn test_combined_apm_function_accessible() {
    // Test that combine_apm_stats is accessible
    use crate::apm::{combine_apm_stats, APMStats};
    
    // Create test data and verify the function works
    let cli_stats = APMStats::default();
    let sdk_stats = APMStats::default();
    let combined = combine_apm_stats(cli_stats, sdk_stats);
    
    // Verify it returns valid data
    assert_eq!(combined.total_sessions, 0);
    assert_eq!(combined.total_messages, 0);
}