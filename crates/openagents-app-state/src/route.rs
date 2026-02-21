use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AppRoute {
    Home,
    Chat {
        thread_id: Option<String>,
    },
    Workers,
    Account {
        #[serde(default)]
        section: Option<String>,
    },
    Settings {
        #[serde(default)]
        section: Option<String>,
    },
    Billing {
        #[serde(default)]
        section: Option<String>,
    },
    Admin {
        #[serde(default)]
        section: Option<String>,
    },
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
        if let Some(section) = section_from_prefix(&path, "/account") {
            return Self::Account { section };
        }
        if path == "/workers" {
            return Self::Workers;
        }
        if let Some(section) = section_from_prefix(&path, "/settings") {
            return Self::Settings { section };
        }
        if let Some(section) = section_from_prefix(&path, "/l402")
            .or_else(|| section_from_prefix(&path, "/billing"))
        {
            return Self::Billing { section };
        }
        if let Some(section) = section_from_prefix(&path, "/admin") {
            return Self::Admin { section };
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
            Self::Account { section } => section_to_path("/account", section),
            Self::Settings { section } => section_to_path("/settings", section),
            Self::Billing { section } => section_to_path("/l402", section),
            Self::Admin { section } => section_to_path("/admin", section),
            Self::Debug => "/debug".to_string(),
            Self::Chat { thread_id: None } => "/chat".to_string(),
            Self::Chat {
                thread_id: Some(thread_id),
            } => format!("/chat/{thread_id}"),
        }
    }
}

fn section_from_prefix(path: &str, prefix: &str) -> Option<Option<String>> {
    if path == prefix {
        return Some(None);
    }
    let prefixed = format!("{prefix}/");
    path.strip_prefix(&prefixed).map(|section| {
        if section.trim().is_empty() {
            None
        } else {
            Some(section.to_string())
        }
    })
}

fn section_to_path(prefix: &str, section: &Option<String>) -> String {
    match section {
        Some(section) => format!("{prefix}/{section}"),
        None => prefix.to_string(),
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

#[cfg(test)]
mod tests {
    use super::AppRoute;

    #[test]
    fn parses_account_settings_and_admin_routes() {
        assert_eq!(
            AppRoute::from_path("/account"),
            AppRoute::Account { section: None }
        );
        assert_eq!(
            AppRoute::from_path("/account/session"),
            AppRoute::Account {
                section: Some("session".to_string())
            }
        );
        assert_eq!(
            AppRoute::from_path("/settings/profile"),
            AppRoute::Settings {
                section: Some("profile".to_string())
            }
        );
        assert_eq!(
            AppRoute::from_path("/l402/transactions"),
            AppRoute::Billing {
                section: Some("transactions".to_string())
            }
        );
        assert_eq!(
            AppRoute::from_path("/billing/paywalls"),
            AppRoute::Billing {
                section: Some("paywalls".to_string())
            }
        );
        assert_eq!(
            AppRoute::from_path("/admin"),
            AppRoute::Admin { section: None }
        );
        assert_eq!(
            AppRoute::from_path("/admin/ops/runtime"),
            AppRoute::Admin {
                section: Some("ops/runtime".to_string())
            }
        );
    }

    #[test]
    fn account_settings_and_admin_routes_round_trip() {
        let routes = vec![
            AppRoute::Account {
                section: Some("profile".to_string()),
            },
            AppRoute::Settings {
                section: Some("integrations".to_string()),
            },
            AppRoute::Billing {
                section: Some("settlements".to_string()),
            },
            AppRoute::Admin {
                section: Some("operators".to_string()),
            },
        ];

        for route in routes {
            let path = route.to_path();
            assert_eq!(AppRoute::from_path(&path), route);
        }
    }
}
