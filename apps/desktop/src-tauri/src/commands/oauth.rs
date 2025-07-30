use serde::{Deserialize, Serialize};
use tauri::{command, AppHandle, Emitter};
use tauri_plugin_oauth::start;
use std::collections::HashMap;
use log::{info, error};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OAuthResult {
    pub code: String,
    pub state: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthTokens {
    pub access_token: String,
    pub token_type: String,
    pub scope: Option<String>,
}

/// Start OAuth server and get the port for callback URL
#[command]
pub async fn start_oauth_server(app_handle: AppHandle) -> Result<u16, String> {
    info!("🔐 [OAUTH] Starting OAuth callback server");
    
    let oauth_result = std::sync::Arc::new(std::sync::Mutex::new(None));
    let oauth_result_clone = oauth_result.clone();
    
    let port_result = start(move |url| {
        info!("🔗 [OAUTH] Received callback URL: {}", url);
        
        // Parse the callback URL to extract code and state
        if let Ok(parsed_url) = url::Url::parse(&url) {
            let mut code = None;
            let mut state = None;
            
            for (key, value) in parsed_url.query_pairs() {
                match key.as_ref() {
                    "code" => code = Some(value.to_string()),
                    "state" => state = Some(value.to_string()),
                    _ => {}
                }
            }
            
            if let Some(auth_code) = code {
                info!("✅ [OAUTH] Successfully extracted authorization code");
                let result = OAuthResult {
                    code: auth_code,
                    state,
                };
                
                // Store result for the main thread
                if let Ok(mut oauth_result) = oauth_result_clone.lock() {
                    *oauth_result = Some(Ok(result.clone()));
                }
                
                // Emit event to frontend with the OAuth result
                let _ = app_handle.emit("oauth_success", &result);
            } else {
                error!("❌ [OAUTH] No authorization code found in callback URL");
                if let Ok(mut oauth_result) = oauth_result_clone.lock() {
                    *oauth_result = Some(Err("No authorization code found".to_string()));
                }
                let _ = app_handle.emit("oauth_error", "No authorization code received");
            }
        } else {
            error!("❌ [OAUTH] Failed to parse callback URL: {}", url);
            if let Ok(mut oauth_result) = oauth_result_clone.lock() {
                *oauth_result = Some(Err("Failed to parse callback URL".to_string()));
            }
            let _ = app_handle.emit("oauth_error", "Invalid callback URL");
        }
    });
    
    match port_result {
        Ok(port) => {
            info!("🌐 [OAUTH] OAuth server started on port {}", port);
            Ok(port)
        }
        Err(err) => {
            error!("💥 [OAUTH] Failed to start OAuth server: {}", err);
            Err(format!("Failed to start OAuth server: {}", err))
        }
    }
}

/// Wait for OAuth callback and return the result
#[command]
pub async fn wait_for_oauth_callback() -> Result<OAuthResult, String> {
    info!("⏳ [OAUTH] Waiting for OAuth callback...");
    
    // This is a simplified approach - in a real implementation, you'd want to
    // share state between start_oauth_server and this function
    
    // For now, we'll use events to handle this
    Err("Use event-based approach instead".to_string())
}

/// Exchange OAuth code for access token and user info
#[command]
pub async fn exchange_oauth_code(
    code: String,
    client_id: String,
    redirect_uri: String,
) -> Result<serde_json::Value, String> {
    info!("🔄 [OAUTH] Exchanging authorization code for access token");
    
    // Create HTTP client
    let client = reqwest::Client::new();
    
    // Prepare token exchange request
    let mut params = HashMap::new();
    params.insert("client_id", client_id);
    params.insert("code", code);
    params.insert("redirect_uri", redirect_uri);
    
    // Make token exchange request to auth service
    let auth_url = std::env::var("VITE_OPENAUTH_URL")
        .unwrap_or_else(|_| "https://auth.openagents.com".to_string());
    
    let token_url = format!("{}/token", auth_url);
    
    match client
        .post(&token_url)
        .form(&params)
        .header("Accept", "application/json")
        .send()
        .await
    {
        Ok(response) => {
            if response.status().is_success() {
                match response.json::<serde_json::Value>().await {
                    Ok(token_response) => {
                        info!("✅ [OAUTH] Successfully exchanged code for tokens and user info");
                        Ok(token_response)
                    }
                    Err(err) => {
                        error!("❌ [OAUTH] Failed to parse token response: {}", err);
                        Err(format!("Failed to parse token response: {}", err))
                    }
                }
            } else {
                let status = response.status();
                let body = response.text().await.unwrap_or_default();
                error!("❌ [OAUTH] Token exchange failed with status {}: {}", status, body);
                Err(format!("Token exchange failed: {} - {}", status, body))
            }
        }
        Err(err) => {
            error!("❌ [OAUTH] Failed to make token exchange request: {}", err);
            Err(format!("Failed to make token exchange request: {}", err))
        }
    }
}

/// Get user info from auth service using access token
#[command]
pub async fn get_user_info(access_token: String) -> Result<serde_json::Value, String> {
    info!("👤 [OAUTH] Fetching user info with access token");
    
    let client = reqwest::Client::new();
    
    let auth_url = std::env::var("VITE_OPENAUTH_URL")
        .unwrap_or_else(|_| "https://auth.openagents.com".to_string());
    
    let user_url = format!("{}/user", auth_url);
    
    match client
        .get(&user_url)
        .bearer_auth(access_token)
        .send()
        .await
    {
        Ok(response) => {
            if response.status().is_success() {
                match response.json::<serde_json::Value>().await {
                    Ok(user_info) => {
                        info!("✅ [OAUTH] Successfully fetched user info");
                        Ok(user_info)
                    }
                    Err(err) => {
                        error!("❌ [OAUTH] Failed to parse user info: {}", err);
                        Err(format!("Failed to parse user info: {}", err))
                    }
                }
            } else {
                let status = response.status();
                let body = response.text().await.unwrap_or_default();
                error!("❌ [OAUTH] Failed to fetch user info: {} - {}", status, body);
                Err(format!("Failed to fetch user info: {} - {}", status, body))
            }
        }
        Err(err) => {
            error!("❌ [OAUTH] Failed to make user info request: {}", err);
            Err(format!("Failed to make user info request: {}", err))
        }
    }
}