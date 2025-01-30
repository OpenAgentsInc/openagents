use anyhow::Result;
use openagents::solver::changes::{generate_changes, parse_search_replace};
use openagents::solver::types::ChangeError;

#[tokio::test]
async fn test_change_generation() -> Result<()> {
    let (changes, reasoning) = generate_changes(
        "src/lib.rs",
        "pub fn add(a: i32, b: i32) -> i32 { a + b }",
        "Add multiply function",
        "Add a multiply function that multiplies two integers",
        "test_url",
    )
    .await?;

    // Verify changes
    assert_eq!(changes.len(), 1);
    assert_eq!(changes[0].path, "src/lib.rs");
    assert!(changes[0].replace.contains("multiply"));
    assert!(changes[0].replace.contains("add")); // Original function preserved
    assert!(!reasoning.is_empty());

    Ok(())
}

#[tokio::test]
async fn test_change_generation_no_changes() -> Result<()> {
    let (changes, reasoning) = generate_changes(
        "src/main.rs",
        "fn main() { println!(\"Hello\"); }",
        "Add multiply function",
        "Add a multiply function to lib.rs",
        "test_url",
    )
    .await?;

    assert!(changes.is_empty());
    assert_eq!(reasoning, "No changes needed");

    Ok(())
}

#[test]
fn test_parse_search_replace_blocks() -> Result<()> {
    let content = r#"src/lib.rs:
<<<<<<< SEARCH
pub fn add(a: i32, b: i32) -> i32 { a + b }
=======
pub fn add(a: i32, b: i32) -> i32 { a + b }

pub fn multiply(a: i32, b: i32) -> i32 { a * b }
>>>>>>> REPLACE"#;

    let changes = parse_search_replace(content)?;

    assert_eq!(changes.len(), 1);
    assert_eq!(changes[0].path, "src/lib.rs");
    assert_eq!(
        changes[0].search,
        "pub fn add(a: i32, b: i32) -> i32 { a + b }"
    );
    assert!(changes[0].replace.contains("multiply"));
    assert!(changes[0].replace.contains("add")); // Original function preserved

    Ok(())
}

#[test]
fn test_parse_search_replace_multiple_files() -> Result<()> {
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

    let changes = parse_search_replace(content)?;

    assert_eq!(changes.len(), 2);
    assert_eq!(changes[0].path, "src/lib.rs");
    assert_eq!(changes[1].path, "src/main.rs");
    assert!(changes[0].replace.contains("multiply"));
    assert!(changes[1].replace.contains("multiply"));

    Ok(())
}

#[test]
fn test_parse_search_replace_new_file() -> Result<()> {
    let content = r#"src/multiply.rs:
<<<<<<< SEARCH
=======
pub fn multiply(a: i32, b: i32) -> i32 {
    a * b
}
>>>>>>> REPLACE"#;

    let changes = parse_search_replace(content)?;

    assert_eq!(changes.len(), 1);
    assert_eq!(changes[0].path, "src/multiply.rs");
    assert!(changes[0].search.is_empty());
    assert!(changes[0].replace.contains("multiply"));

    Ok(())
}

#[test]
fn test_parse_search_replace_invalid() -> Result<()> {
    let content = r#"src/lib.rs:
Invalid format without proper markers"#;

    let result = parse_search_replace(content);
    assert!(matches!(result, Err(e) if e.to_string().contains("No SEARCH marker found")));

    Ok(())
}

#[test]
fn test_parse_search_replace_missing_path() -> Result<()> {
    let content = r#"<<<<<<< SEARCH
some content
=======
new content
>>>>>>> REPLACE"#;

    let result = parse_search_replace(content);
    assert!(matches!(result, Err(ChangeError::InvalidFormat)));

    Ok(())
}
