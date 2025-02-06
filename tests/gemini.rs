use anyhow::Result;
use openagents::server::services::gemini::GeminiService;
use std::env;

#[tokio::test]
async fn test_gemini_file_analysis() -> Result<()> {
    // Load environment variables from .env file
    dotenvy::dotenv().ok();

    // Skip test if GEMINI_API_KEY is not set
    if env::var("GEMINI_API_KEY").is_err() {
        println!("Skipping test_gemini_file_analysis: GEMINI_API_KEY not set");
        return Ok(());
    }

    let gemini = GeminiService::new()?;

    let issue_description = "Add a new endpoint for user profile updates";
    let valid_paths = vec![
        "src/routes.rs".to_string(),
        "src/handlers/user.rs".to_string(),
        "src/models/user.rs".to_string(),
        "tests/user.rs".to_string(),
    ];
    let repo_context = "A Rust web application using axum for routing and PostgreSQL for storage.";

    let response = gemini
        .analyze_files(issue_description, &valid_paths, repo_context)
        .await?;

    // Verify response structure
    assert!(response.is_object());
    let files = response.get("files").expect("files array not found");
    assert!(files.is_array());

    let files_array = files.as_array().unwrap();
    assert!(!files_array.is_empty());

    // Check first file has required fields
    let first_file = &files_array[0];
    assert!(first_file.get("path").is_some());
    assert!(first_file.get("relevance_score").is_some());
    assert!(first_file.get("reason").is_some());

    Ok(())
}