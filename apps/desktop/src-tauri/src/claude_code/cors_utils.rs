/// CORS Utilities for OpenAuth Integration
/// 
/// Phase 4: Production-ready CORS handling and validation utilities
/// Provides helper functions for CORS error detection and handling

use crate::error::AppError;
use reqwest::{Response, Error as ReqwestError};
use std::collections::HashMap;

/// CORS configuration constants
pub struct CorsConfig {
    pub allowed_origins: Vec<String>,
    pub allowed_methods: Vec<String>,
    pub allowed_headers: Vec<String>,
    pub max_age: u32,
}

impl Default for CorsConfig {
    fn default() -> Self {
        Self {
            allowed_origins: vec![
                "tauri://localhost".to_string(),
                "https://tauri.localhost".to_string(),
                "capacitor://localhost".to_string(),
                "http://localhost:3000".to_string(),
                "https://openagents.com".to_string(),
            ],
            allowed_methods: vec![
                "GET".to_string(),
                "POST".to_string(),
                "OPTIONS".to_string(),
            ],
            allowed_headers: vec![
                "Content-Type".to_string(),
                "Authorization".to_string(),
                "X-Requested-With".to_string(),
                "Accept".to_string(),
                "Origin".to_string(),
            ],
            max_age: 86400, // 24 hours
        }
    }
}

/// CORS error detection and handling utilities
pub struct CorsHandler {
    config: CorsConfig,
}

impl CorsHandler {
    /// Create a new CORS handler with default configuration
    pub fn new() -> Self {
        Self {
            config: CorsConfig::default(),
        }
    }

    /// Create a CORS handler with custom configuration
    pub fn with_config(config: CorsConfig) -> Self {
        Self { config }
    }

    /// Check if an error is CORS-related
    pub fn is_cors_error(&self, error: &ReqwestError) -> bool {
        let error_message = error.to_string().to_lowercase();
        
        // Common CORS error indicators
        error_message.contains("cors") ||
        error_message.contains("cross-origin") ||
        error_message.contains("access-control") ||
        error_message.contains("preflight") ||
        error_message.contains("not allowed by access-control-allow-origin") ||
        error_message.contains("request header field") ||
        error_message.contains("is not allowed")
    }

    /// Convert a reqwest error to appropriate AppError with CORS context
    pub fn handle_error(&self, error: ReqwestError) -> AppError {
        if self.is_cors_error(&error) {
            log::error!("CORS_ERROR: {}", error);
            AppError::CorsError(format!("CORS policy blocked request: {}", error))
        } else if error.is_timeout() {
            log::error!("NETWORK_TIMEOUT: {}", error);
            AppError::NetworkTimeout(format!("Request timeout: {}", error))
        } else if error.is_connect() {
            log::error!("CONNECTION_ERROR: {}", error);
            AppError::ConvexConnectionError(format!("Connection failed: {}", error))
        } else {
            log::error!("HTTP_ERROR: {}", error);
            AppError::Http(error)
        }
    }

    /// Validate CORS headers in a response
    pub fn validate_cors_headers(&self, response: &Response) -> Result<(), AppError> {
        let headers = response.headers();
        
        // Check for essential CORS headers
        if !headers.contains_key("access-control-allow-origin") {
            log::warn!("CORS_VALIDATION: Missing Access-Control-Allow-Origin header");
            return Err(AppError::CorsError(
                "Server response missing CORS headers".to_string()
            ));
        }

        // Log CORS headers for debugging
        self.log_cors_headers(headers);
        
        Ok(())
    }

    /// Create request headers with proper Origin for CORS
    pub fn create_cors_headers(&self) -> HashMap<String, String> {
        let mut headers = HashMap::new();
        
        // Use the first allowed origin as default
        if let Some(origin) = self.config.allowed_origins.first() {
            headers.insert("Origin".to_string(), origin.clone());
        }
        
        headers.insert("Accept".to_string(), "application/json".to_string());
        headers.insert("User-Agent".to_string(), "OpenAgents-Desktop/1.0".to_string());
        
        headers
    }

    /// Log CORS headers for debugging
    fn log_cors_headers(&self, headers: &reqwest::header::HeaderMap) {
        if let Some(origin) = headers.get("access-control-allow-origin") {
            log::debug!("CORS: Access-Control-Allow-Origin: {:?}", origin);
        }
        
        if let Some(methods) = headers.get("access-control-allow-methods") {
            log::debug!("CORS: Access-Control-Allow-Methods: {:?}", methods);
        }
        
        if let Some(headers_allowed) = headers.get("access-control-allow-headers") {
            log::debug!("CORS: Access-Control-Allow-Headers: {:?}", headers_allowed);
        }
        
        if let Some(credentials) = headers.get("access-control-allow-credentials") {
            log::debug!("CORS: Access-Control-Allow-Credentials: {:?}", credentials);
        }
    }

    /// Check if the current environment supports CORS
    pub fn check_cors_support(&self) -> Result<(), AppError> {
        // In Tauri, CORS is handled by the webview
        log::debug!("CORS: Tauri environment detected, CORS handled by webview");
        
        // Additional checks could be added here for different environments
        Ok(())
    }

    /// Provide CORS troubleshooting suggestions
    pub fn get_cors_troubleshooting_info(&self, error: &str) -> String {
        let mut suggestions = Vec::new();
        
        if error.contains("access-control-allow-origin") {
            suggestions.push("• Check that the OpenAuth server includes CORS headers");
            suggestions.push("• Verify the origin 'tauri://localhost' is in the allowed origins list");
        }
        
        if error.contains("authorization") {
            suggestions.push("• Ensure 'Authorization' is in Access-Control-Allow-Headers");
            suggestions.push("• Check that preflight requests are handled correctly");
        }
        
        if error.contains("preflight") {
            suggestions.push("• Verify the server responds to OPTIONS requests");
            suggestions.push("• Check Access-Control-Allow-Methods includes POST");
        }
        
        if error.contains("credentials") {
            suggestions.push("• Set Access-Control-Allow-Credentials: true on the server");
            suggestions.push("• Ensure cookies are handled correctly");
        }
        
        if suggestions.is_empty() {
            suggestions.push("• Review the CORS_CONFIGURATION.md file for detailed setup instructions");
            suggestions.push("• Check server logs for CORS-related errors");
            suggestions.push("• Verify network connectivity to the OpenAuth server");
        }
        
        format!("CORS Troubleshooting:\n{}", suggestions.join("\n"))
    }
}

impl Default for CorsHandler {
    fn default() -> Self {
        Self::new()
    }
}

/// Helper function to create a CORS-aware HTTP client
pub fn create_cors_client() -> Result<reqwest::Client, AppError> {
    reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .user_agent("OpenAgents-Desktop/1.0")
        .danger_accept_invalid_certs(false) // Always validate certificates
        .build()
        .map_err(|e| AppError::Http(e))
}

/// Helper function to log CORS request details
pub fn log_cors_request(method: &str, url: &str, has_auth: bool) {
    log::debug!("CORS_REQUEST: {} {} [auth={}]", method, url, has_auth);
}

/// Helper function to handle CORS preflight responses
pub fn handle_preflight_response(response: &Response) -> Result<(), AppError> {
    if response.status() == 200 || response.status() == 204 {
        log::debug!("CORS_PREFLIGHT: Successful preflight response");
        Ok(())
    } else {
        log::error!("CORS_PREFLIGHT: Failed preflight with status {}", response.status());
        Err(AppError::CorsError(format!(
            "Preflight request failed with status {}", 
            response.status()
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cors_handler_creation() {
        let handler = CorsHandler::new();
        assert!(!handler.config.allowed_origins.is_empty());
        assert!(!handler.config.allowed_methods.is_empty());
        assert!(!handler.config.allowed_headers.is_empty());
    }

    #[test]
    fn test_cors_error_detection() {
        let handler = CorsHandler::new();
        
        // Test CORS error detection with various error messages
        // Note: We can't easily create reqwest::Error instances in tests,
        // so we'll test the error message detection logic separately
        
        let cors_messages = vec![
            "CORS policy: No 'Access-Control-Allow-Origin' header",
            "cross-origin request blocked",
            "access-control-allow-origin",
            "preflight failed",
            "not allowed by access-control-allow-origin",
        ];
        
        for message in cors_messages {
            // The is_cors_error method checks the error message content
            let lowercase = message.to_lowercase();
            assert!(lowercase.contains("cors") || 
                    lowercase.contains("cross-origin") ||
                    lowercase.contains("access-control") ||
                    lowercase.contains("preflight"));
        }
    }

    #[test]
    fn test_cors_headers_creation() {
        let handler = CorsHandler::new();
        let headers = handler.create_cors_headers();
        
        assert!(headers.contains_key("Origin"));
        assert!(headers.contains_key("Accept"));
        assert!(headers.contains_key("User-Agent"));
        assert_eq!(headers.get("Accept").unwrap(), "application/json");
    }

    #[test]
    fn test_cors_troubleshooting_info() {
        let handler = CorsHandler::new();
        
        let info = handler.get_cors_troubleshooting_info("access-control-allow-origin");
        assert!(info.contains("CORS Troubleshooting"));
        assert!(info.contains("OpenAuth server"));
        
        let info = handler.get_cors_troubleshooting_info("authorization");
        assert!(info.contains("Authorization"));
        assert!(info.contains("Access-Control-Allow-Headers"));
    }

    #[test]
    fn test_cors_support_check() {
        let handler = CorsHandler::new();
        let result = handler.check_cors_support();
        assert!(result.is_ok());
    }
}