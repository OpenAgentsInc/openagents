//! Vibe configuration and project templates

use serde::{Deserialize, Serialize};

/// Configuration for a Vibe project
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VibeConfig {
    /// Frontend configuration
    pub frontend: FrontendConfig,
    /// Backend configuration (optional)
    pub backend: Option<BackendConfig>,
    /// Route mappings
    pub routes: Vec<RouteMapping>,
}

/// Frontend configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FrontendConfig {
    /// Entry point (e.g., "frontend/index.html")
    pub entry: String,
    /// Framework preset
    pub framework: FrameworkPreset,
}

/// Backend configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackendConfig {
    /// Path to Rust crate (e.g., "backend")
    pub crate_path: String,
    /// Router function name (e.g., "backend::routes::router")
    pub router: String,
}

/// Route mapping for API endpoints
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouteMapping {
    /// Path pattern (e.g., "/api/hello")
    pub path: String,
    /// HTTP method
    pub method: HttpMethod,
    /// Handler function name
    pub handler: String,
}

/// HTTP method
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum HttpMethod {
    Get,
    Post,
    Put,
    Delete,
    Patch,
}

/// Framework preset
#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum FrameworkPreset {
    #[default]
    React,
    Vue,
    Svelte,
    Solid,
    Plain,
}

impl VibeConfig {
    /// Create a frontend-only React template
    pub fn react_template() -> Self {
        Self {
            frontend: FrontendConfig {
                entry: "frontend/index.html".into(),
                framework: FrameworkPreset::React,
            },
            backend: None,
            routes: vec![],
        }
    }

    /// Create a full-stack React + Rust template
    pub fn fullstack_template() -> Self {
        Self {
            frontend: FrontendConfig {
                entry: "frontend/index.html".into(),
                framework: FrameworkPreset::React,
            },
            backend: Some(BackendConfig {
                crate_path: "backend".into(),
                router: "backend::routes::router".into(),
            }),
            routes: vec![RouteMapping {
                path: "/api/hello".into(),
                method: HttpMethod::Get,
                handler: "backend::routes::hello".into(),
            }],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_react_template() {
        let config = VibeConfig::react_template();
        assert_eq!(config.frontend.entry, "frontend/index.html");
        assert!(config.backend.is_none());
    }

    #[test]
    fn test_fullstack_template() {
        let config = VibeConfig::fullstack_template();
        assert!(config.backend.is_some());
        assert!(!config.routes.is_empty());
    }
}
