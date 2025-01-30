use anyhow::{anyhow, Result};
use serde_json::json;
use tracing::debug;

#[cfg(test)]
mod tests {
    use super::*;
    use mockito::mock;

    #[tokio::test]
    async fn test_generate_pr_title() {
        let mock_response = json!({
            "choices": [{
                "message": {
                    "content": "feat: add multiply function"
                }
            }]
        });

        let _m = mock("POST", "/v1/chat/completions")
            .with_status(200)
            .with_header("content-type", "application/json")
            .with_body(mock_response.to_string())
            .create();

        let result = generate_pr_title(123, "Add multiply function").await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "feat: add multiply function");
    }
}