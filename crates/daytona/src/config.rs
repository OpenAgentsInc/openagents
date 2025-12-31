//! Configuration for the Daytona API client.

use std::time::Duration;

/// Configuration for connecting to the Daytona API.
#[derive(Debug, Clone)]
pub struct DaytonaConfig {
    /// Base URL for the Daytona API (e.g., "https://api.daytona.io")
    pub base_url: String,

    /// API key for authentication (mutually exclusive with bearer_token)
    pub api_key: Option<String>,

    /// Bearer token (JWT) for authentication (mutually exclusive with api_key)
    pub bearer_token: Option<String>,

    /// Organization ID to use with JWT authentication
    /// Sent as X-Daytona-Organization-ID header
    pub organization_id: Option<String>,

    /// HTTP request timeout
    pub timeout: Duration,
}

impl Default for DaytonaConfig {
    fn default() -> Self {
        Self {
            base_url: "https://api.daytona.io".to_string(),
            api_key: None,
            bearer_token: None,
            organization_id: None,
            timeout: Duration::from_secs(30),
        }
    }
}

impl DaytonaConfig {
    /// Create a new configuration with an API key.
    pub fn with_api_key(api_key: impl Into<String>) -> Self {
        Self {
            api_key: Some(api_key.into()),
            ..Default::default()
        }
    }

    /// Create a new configuration with a bearer token.
    pub fn with_bearer_token(token: impl Into<String>) -> Self {
        Self {
            bearer_token: Some(token.into()),
            ..Default::default()
        }
    }

    /// Set the base URL.
    pub fn base_url(mut self, url: impl Into<String>) -> Self {
        self.base_url = url.into();
        self
    }

    /// Set the organization ID.
    pub fn organization_id(mut self, org_id: impl Into<String>) -> Self {
        self.organization_id = Some(org_id.into());
        self
    }

    /// Set the request timeout.
    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }
}
