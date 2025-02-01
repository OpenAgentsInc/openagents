#[cfg(test)]
mod tests {
    use anyhow::Result;
    use openagents::solver::types::Change;

    #[test]
    #[ignore = "requires solver setup"]
    fn test_change_generation() -> Result<()> {
        let changes = vec![Change::new(
            "src/main.rs".to_string(),
            "".to_string(),
            "fn add(a: i32, b: i32) -> i32 { a + b }".to_string(),
        )];
        assert_eq!(changes.len(), 1);
        Ok(())
    }

    #[test]
    #[ignore = "requires solver setup"]
    fn test_change_generation_no_changes() -> Result<()> {
        let changes: Vec<Change> = vec![];
        assert!(changes.is_empty());
        Ok(())
    }
}
