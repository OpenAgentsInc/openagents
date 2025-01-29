use super::*;
use crate::solver::types::{ChangeError, ChangeResult};
use anyhow::Result;

#[tokio::test]
async fn test_generate_changes() -> Result<()> {
    let (changes, reasoning) = generation::generate_changes(
        "src/lib.rs",
        "pub fn add(a: i32, b: i32) -> i32 { a + b }",
        "Add multiply function",
        "Add a multiply function that multiplies two integers",
        "test_key",
    )
    .await?;

    assert_eq!(changes.len(), 1);
    assert_eq!(changes[0].path, "src/lib.rs");
    assert!(changes[0].replace.contains("multiply"));
    assert!(!reasoning.is_empty());

    Ok(())
}

#[tokio::test]
async fn test_generate_changes_no_changes() -> Result<()> {
    let (changes, reasoning) = generation::generate_changes(
        "src/main.rs",
        "fn main() { println!(\"Hello\"); }",
        "Add multiply function",
        "Add a multiply function to lib.rs",
        "test_key",
    )
    .await?;

    assert!(changes.is_empty());
    assert_eq!(reasoning, "No changes needed");

    Ok(())
}

#[test]
fn test_parse_search_replace() -> ChangeResult<()> {
    let content = r#"src/lib.rs:
<<<<<<< SEARCH
pub fn add(a: i32, b: i32) -> i32 { a + b }
=======
pub fn add(a: i32, b: i32) -> i32 { a + b }

pub fn multiply(a: i32, b: i32) -> i32 { a * b }
>>>>>>> REPLACE"#;

    let changes = parsing::parse_search_replace(content)?;
    assert_eq!(changes.len(), 1);
    assert_eq!(changes[0].path, "src/lib.rs");
    assert!(changes[0].replace.contains("multiply"));

    Ok(())
}

#[test]
fn test_parse_search_replace_multiple() -> ChangeResult<()> {
    let content = r#"src/lib.rs:
<<<<<<< SEARCH
pub fn add(a: i32, b: i32) -> i32 { a + b }
=======
pub fn add(a: i32, b: i32) -> i32 { a + b }

pub fn multiply(a: i32, b: i32) -> i32 { a * b }
>>>>>>> REPLACE

src/main.rs:
<<<<<<< SEARCH
fn main() {
    println!("1 + 1 = {}", add(1, 1));
}
=======
fn main() {
    println!("1 + 1 = {}", add(1, 1));
    println!("2 * 3 = {}", multiply(2, 3));
}
>>>>>>> REPLACE"#;

    let changes = parsing::parse_search_replace(content)?;
    assert_eq!(changes.len(), 2);
    assert_eq!(changes[0].path, "src/lib.rs");
    assert_eq!(changes[1].path, "src/main.rs");
    assert!(changes[0].replace.contains("multiply"));
    assert!(changes[1].replace.contains("multiply"));

    Ok(())
}

#[test]
fn test_parse_search_replace_invalid() -> ChangeResult<()> {
    let content = r#"src/lib.rs:
Invalid format without proper markers"#;

    let result = parsing::parse_search_replace(content);
    assert!(matches!(result, Err(ChangeError::InvalidFormat)));

    Ok(())
}
