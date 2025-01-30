use serde::Deserialize;

/// Response from LLM for change generation
#[derive(Debug, Deserialize)]
pub struct ChangeResponse {
    pub changes: Vec<ChangeBlock>,
    pub reasoning: String,
}

/// A block of changes from the LLM
#[derive(Debug, Deserialize, Clone)]
pub struct ChangeBlock {
    pub path: String,
    pub search: String,
    pub replace: String,
    pub reason: String,
}

impl ChangeBlock {
    pub fn validate(&self) -> bool {
        !self.path.is_empty() && 
        !self.reason.is_empty() && 
        (!self.search.is_empty() || !self.replace.is_empty())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_change_block_validation() {
        let valid = ChangeBlock {
            path: "test.rs".to_string(),
            search: "old".to_string(),
            replace: "new".to_string(),
            reason: "Update test".to_string(),
        };
        assert!(valid.validate());

        let invalid_path = ChangeBlock {
            path: "".to_string(),
            search: "old".to_string(),
            replace: "new".to_string(),
            reason: "Update test".to_string(),
        };
        assert!(!invalid_path.validate());

        let invalid_reason = ChangeBlock {
            path: "test.rs".to_string(),
            search: "old".to_string(),
            replace: "new".to_string(),
            reason: "".to_string(),
        };
        assert!(!invalid_reason.validate());
    }
}