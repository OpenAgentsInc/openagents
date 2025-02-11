use openagents::solver::types::Change;

#[test]
fn test_change_validation() {
    // Valid change with non-empty search and replace
    let change = Change::new(
        "src/main.rs".to_string(),
        "fn old()".to_string(),
        "fn new()".to_string(),
    );
    assert!(
        change.validate().is_ok(),
        "Valid change with path '{}', search '{}', replace '{}' should be ok",
        change.path,
        change.search,
        change.replace
    );

    // Empty path
    let change = Change::new(
        "".to_string(),
        "fn old()".to_string(),
        "fn new()".to_string(),
    );
    assert!(matches!(change.validate(), Err(e) if e.to_string().contains("Path cannot be empty")));

    // Empty content
    let change = Change::new("src/main.rs".to_string(), "".to_string(), "".to_string());
    assert!(
        matches!(change.validate(), Err(e) if e.to_string().contains("Search content cannot be empty"))
    );

    // Empty search but non-empty replace (valid for new file)
    let change = Change::new(
        "src/main.rs".to_string(),
        "".to_string(),
        "fn new()".to_string(),
    );
    assert!(change.validate().is_ok());

    // Non-empty search but empty replace (valid for deletion)
    let change = Change::new(
        "src/main.rs".to_string(),
        "fn old()".to_string(),
        "".to_string(),
    );
    assert!(change.validate().is_ok());
}

#[test]
fn test_change_equality() {
    let change1 = Change::new(
        "src/main.rs".to_string(),
        "fn old()".to_string(),
        "fn new()".to_string(),
    );

    let change2 = Change::new(
        "src/main.rs".to_string(),
        "fn old()".to_string(),
        "fn new()".to_string(),
    );

    let change3 = Change::new(
        "src/lib.rs".to_string(),
        "fn old()".to_string(),
        "fn new()".to_string(),
    );

    assert_eq!(change1, change2);
    assert_ne!(change1, change3);
}
