use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AppRoute {
    Home,
    Chat { thread_id: Option<String> },
    Workers,
    Settings,
    Debug,
}

impl Default for AppRoute {
    fn default() -> Self {
        Self::Home
    }
}

impl AppRoute {
    pub fn from_path(raw_path: &str) -> Self {
        let path = normalize_path(raw_path);
        if path == "/" {
            return Self::Home;
        }
        if path == "/workers" {
            return Self::Workers;
        }
        if path == "/settings" {
            return Self::Settings;
        }
        if path == "/debug" {
            return Self::Debug;
        }
        if path == "/chat" {
            return Self::Chat { thread_id: None };
        }
        if let Some(thread_id) = path.strip_prefix("/chat/") {
            return Self::Chat {
                thread_id: Some(thread_id.to_string()),
            };
        }
        Self::Home
    }

    pub fn to_path(&self) -> String {
        match self {
            Self::Home => "/".to_string(),
            Self::Workers => "/workers".to_string(),
            Self::Settings => "/settings".to_string(),
            Self::Debug => "/debug".to_string(),
            Self::Chat { thread_id: None } => "/chat".to_string(),
            Self::Chat {
                thread_id: Some(thread_id),
            } => format!("/chat/{thread_id}"),
        }
    }
}

fn normalize_path(raw_path: &str) -> String {
    let path = raw_path.trim();
    if path.is_empty() || path == "/" {
        return "/".to_string();
    }
    let mut normalized = if path.starts_with('/') {
        path.to_string()
    } else {
        format!("/{path}")
    };
    while normalized.contains("//") {
        normalized = normalized.replace("//", "/");
    }
    if normalized.len() > 1 {
        normalized = normalized.trim_end_matches('/').to_string();
    }
    normalized
}
