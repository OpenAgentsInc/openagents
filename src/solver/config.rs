use anyhow::{anyhow, Result};
use dotenvy::dotenv;
use std::env;

pub struct Config {
    pub openrouter_api_key: String,
    pub github_token: String,
}

impl Config {
    pub fn load() -> Result<Self> {
        // Load .env file first
        if let Err(e) = dotenv() {
            return Err(anyhow!("Failed to load .env file: {}", e));
        }

        // Get API keys immediately and fail if not present
        let openrouter_api_key = env::var("OPENROUTER_API_KEY")
            .map_err(|_| anyhow!("OPENROUTER_API_KEY not found in environment or .env file"))?;

        let github_token = env::var("GITHUB_TOKEN")
            .map_err(|_| anyhow!("GITHUB_TOKEN not found in environment or .env file"))?;

        Ok(Self {
            openrouter_api_key,
            github_token,
        })
    }
}
