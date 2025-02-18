// ... (previous imports stay the same)

impl Default for AppConfig {
    fn default() -> Self {
        // Load .env file if it exists
        dotenvy::dotenv().ok();
        
        // Determine if we're in development mode
        let is_dev = env::var("APP_ENVIRONMENT").unwrap_or_default() != "production";
        
        // Get frontend URL from .env, with different defaults for dev/prod
        let frontend_url = env::var("FRONTEND_URL").unwrap_or_else(|_| {
            if is_dev {
                "http://localhost:5173".to_string()
            } else {
                // In production, default to openagents.com
                "https://openagents.com".to_string()
            }
        });

        Self {
            scramble_auth_url: env::var("SCRAMBLE_AUTH_URL")
                .expect("SCRAMBLE_AUTH_URL must be set"),
            scramble_token_url: env::var("SCRAMBLE_TOKEN_URL")
                .expect("SCRAMBLE_TOKEN_URL must be set"),
            scramble_client_id: env::var("SCRAMBLE_CLIENT_ID")
                .expect("SCRAMBLE_CLIENT_ID must be set"),
            scramble_client_secret: env::var("SCRAMBLE_CLIENT_SECRET")
                .expect("SCRAMBLE_CLIENT_SECRET must be set"),
            scramble_redirect_uri: env::var("SCRAMBLE_REDIRECT_URI")
                .unwrap_or_else(|_| format!("{}/auth/scramble/callback", frontend_url)),
            database_url: env::var("DATABASE_URL").expect("DATABASE_URL must be set"),
            github_client_id: env::var("GITHUB_CLIENT_ID").expect("GITHUB_CLIENT_ID must be set"),
            github_client_secret: env::var("GITHUB_CLIENT_SECRET")
                .expect("GITHUB_CLIENT_SECRET must be set"),
            github_redirect_uri: env::var("GITHUB_REDIRECT_URI")
                .unwrap_or_else(|_| format!("{}/auth/github/callback", frontend_url)),
            frontend_url,
        }
    }
}