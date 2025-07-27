use crate::{calculate_apm, get_tool_category, ProductivityByTime};

#[test]
fn test_calculate_apm_basic() {
    // Test basic APM calculation
    let apm = calculate_apm(10, 5, 30.0); // 10 messages + 5 tools in 30 minutes
    assert_eq!(apm, 0.5); // 15 actions / 30 minutes = 0.5 APM
}

#[test]
fn test_calculate_apm_zero_duration() {
    // Test APM calculation with zero duration
    let apm = calculate_apm(10, 5, 0.0);
    assert_eq!(apm, 0.0); // Should return 0 instead of dividing by zero
}

#[test]
fn test_calculate_apm_no_actions() {
    // Test APM calculation with no actions
    let apm = calculate_apm(0, 0, 60.0);
    assert_eq!(apm, 0.0); // 0 actions / 60 minutes = 0 APM
}

#[test]
fn test_get_tool_category() {
    // Test tool categorization
    assert_eq!(get_tool_category("Edit"), "Code Generation");
    assert_eq!(get_tool_category("MultiEdit"), "Code Generation");
    assert_eq!(get_tool_category("Write"), "Code Generation");
    
    assert_eq!(get_tool_category("Read"), "File Operations");
    assert_eq!(get_tool_category("LS"), "File Operations");
    assert_eq!(get_tool_category("Glob"), "File Operations");
    
    assert_eq!(get_tool_category("Bash"), "System Operations");
    
    assert_eq!(get_tool_category("Grep"), "Search");
    assert_eq!(get_tool_category("WebSearch"), "Search");
    assert_eq!(get_tool_category("WebFetch"), "Search");
    
    assert_eq!(get_tool_category("TodoWrite"), "Planning");
    assert_eq!(get_tool_category("TodoRead"), "Planning");
    
    assert_eq!(get_tool_category("UnknownTool"), "Other");
}

// Note: clean_project_name is a private function and cannot be tested directly

#[test]
fn test_productivity_by_time_default() {
    // Test default productivity values
    let productivity = ProductivityByTime {
        morning: 0.0,
        afternoon: 0.0,
        evening: 0.0,
        night: 0.0,
    };
    
    // Serialize and deserialize to ensure it works
    let json = serde_json::to_string(&productivity).unwrap();
    let deserialized: ProductivityByTime = serde_json::from_str(&json).unwrap();
    
    assert_eq!(deserialized.morning, 0.0);
    assert_eq!(deserialized.afternoon, 0.0);
    assert_eq!(deserialized.evening, 0.0);
    assert_eq!(deserialized.night, 0.0);
}

#[test]
fn test_apm_high_precision() {
    // Test APM calculation with high precision values
    let apm = calculate_apm(123, 456, 789.5);
    let expected = (123.0 + 456.0) / 789.5;
    assert!((apm - expected).abs() < 0.0001); // Allow small floating point differences
}