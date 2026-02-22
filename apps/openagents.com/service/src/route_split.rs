use std::sync::Arc;

use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tokio::sync::RwLock;

use crate::config::Config;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RouteTarget {
    Legacy,
    RustShell,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RouteMode {
    Legacy,
    Rust,
    Cohort,
}

#[derive(Debug, Clone, Serialize)]
pub struct RouteSplitDecision {
    pub path: String,
    pub target: RouteTarget,
    pub reason: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cohort_bucket: Option<u8>,
    pub cohort_key: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct RouteSplitStatus {
    pub enabled: bool,
    pub mode: String,
    pub cohort_percentage: u8,
    pub rust_routes: Vec<String>,
    pub force_legacy: bool,
    pub legacy_base_url: Option<String>,
    pub override_target: Option<RouteTarget>,
}

#[derive(Clone)]
pub struct RouteSplitService {
    config: RouteSplitConfig,
    override_target: Arc<RwLock<Option<RouteTarget>>>,
}

#[derive(Debug, Clone)]
struct RouteSplitConfig {
    enabled: bool,
    mode: RouteMode,
    rust_routes: Vec<String>,
    cohort_percentage: u8,
    salt: String,
    force_legacy: bool,
    legacy_base_url: Option<String>,
}

impl RouteSplitService {
    pub fn from_config(config: &Config) -> Self {
        Self {
            config: RouteSplitConfig::from_config(config),
            override_target: Arc::new(RwLock::new(None)),
        }
    }

    pub async fn evaluate(&self, path: &str, cohort_key: &str) -> RouteSplitDecision {
        let normalized_path = normalize_path(path);
        let normalized_cohort_key = normalize_cohort_key(cohort_key);
        let override_target = *self.override_target.read().await;

        // Keep Codex worker control APIs on the Laravel authority lane until
        // an explicit ownership migration is completed.
        if is_codex_worker_control_path(&normalized_path) {
            return RouteSplitDecision {
                path: normalized_path,
                target: RouteTarget::Legacy,
                reason: "codex_worker_control_legacy_authority".to_string(),
                cohort_bucket: None,
                cohort_key: normalized_cohort_key,
            };
        }

        if self.config.force_legacy {
            return RouteSplitDecision {
                path: normalized_path,
                target: RouteTarget::Legacy,
                reason: "force_legacy".to_string(),
                cohort_bucket: None,
                cohort_key: normalized_cohort_key,
            };
        }

        if let Some(target) = override_target {
            return RouteSplitDecision {
                path: normalized_path,
                target,
                reason: "runtime_override".to_string(),
                cohort_bucket: None,
                cohort_key: normalized_cohort_key,
            };
        }

        if !self.config.enabled {
            return RouteSplitDecision {
                path: normalized_path,
                target: RouteTarget::Legacy,
                reason: "route_split_disabled".to_string(),
                cohort_bucket: None,
                cohort_key: normalized_cohort_key,
            };
        }

        if !self.matches_rust_route(path) {
            return RouteSplitDecision {
                path: normalized_path,
                target: RouteTarget::Legacy,
                reason: "legacy_route_default".to_string(),
                cohort_bucket: None,
                cohort_key: normalized_cohort_key,
            };
        }

        match self.config.mode {
            RouteMode::Legacy => RouteSplitDecision {
                path: normalized_path,
                target: RouteTarget::Legacy,
                reason: "mode_legacy".to_string(),
                cohort_bucket: None,
                cohort_key: normalized_cohort_key,
            },
            RouteMode::Rust => RouteSplitDecision {
                path: normalized_path,
                target: RouteTarget::RustShell,
                reason: "mode_rust".to_string(),
                cohort_bucket: None,
                cohort_key: normalized_cohort_key,
            },
            RouteMode::Cohort => {
                let bucket = stable_bucket(&normalized_cohort_key, &self.config.salt);
                let target = if bucket < self.config.cohort_percentage {
                    RouteTarget::RustShell
                } else {
                    RouteTarget::Legacy
                };

                RouteSplitDecision {
                    path: normalized_path,
                    target,
                    reason: "mode_cohort".to_string(),
                    cohort_bucket: Some(bucket),
                    cohort_key: normalized_cohort_key,
                }
            }
        }
    }

    pub async fn set_override_target(&self, target: Option<RouteTarget>) {
        let mut lock = self.override_target.write().await;
        *lock = target;
    }

    pub async fn status(&self) -> RouteSplitStatus {
        let override_target = *self.override_target.read().await;
        RouteSplitStatus {
            enabled: self.config.enabled,
            mode: self.config.mode.as_str().to_string(),
            cohort_percentage: self.config.cohort_percentage,
            rust_routes: self.config.rust_routes.clone(),
            force_legacy: self.config.force_legacy,
            legacy_base_url: self.config.legacy_base_url.clone(),
            override_target,
        }
    }

    pub fn legacy_redirect_url(&self, path: &str, query: Option<&str>) -> Option<String> {
        let base = self.config.legacy_base_url.as_ref()?;
        if let Some(query) = query.filter(|value| !value.trim().is_empty()) {
            return Some(format!("{base}{path}?{query}"));
        }
        Some(format!("{base}{path}"))
    }

    fn matches_rust_route(&self, path: &str) -> bool {
        let normalized = normalize_path(path);
        self.config.rust_routes.iter().any(|prefix| {
            if prefix == "/" {
                return true;
            }
            normalized == *prefix || normalized.starts_with(&format!("{prefix}/"))
        })
    }
}

impl RouteSplitConfig {
    fn from_config(config: &Config) -> Self {
        Self {
            enabled: config.route_split_enabled,
            mode: RouteMode::from_str(&config.route_split_mode),
            rust_routes: normalize_routes(&config.route_split_rust_routes),
            cohort_percentage: config.route_split_cohort_percentage.min(100),
            salt: config.route_split_salt.clone(),
            force_legacy: config.route_split_force_legacy,
            legacy_base_url: config.route_split_legacy_base_url.clone(),
        }
    }
}

impl RouteMode {
    fn from_str(value: &str) -> Self {
        match value.trim().to_lowercase().as_str() {
            "rust" => Self::Rust,
            "cohort" => Self::Cohort,
            _ => Self::Legacy,
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::Legacy => "legacy",
            Self::Rust => "rust",
            Self::Cohort => "cohort",
        }
    }
}

fn normalize_routes(routes: &[String]) -> Vec<String> {
    let mut normalized = Vec::new();
    for route in routes {
        let path = normalize_path(route);
        if !normalized.iter().any(|existing| existing == &path) {
            normalized.push(path);
        }
    }
    normalized
}

fn normalize_path(path: &str) -> String {
    let trimmed = path.trim();
    if trimmed.is_empty() || trimmed == "/" {
        return "/".to_string();
    }
    let mut normalized = if trimmed.starts_with('/') {
        trimmed.to_string()
    } else {
        format!("/{trimmed}")
    };
    while normalized.contains("//") {
        normalized = normalized.replace("//", "/");
    }
    normalized.trim_end_matches('/').to_string()
}

fn normalize_cohort_key(value: &str) -> String {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        "anonymous".to_string()
    } else {
        trimmed.to_string()
    }
}

fn stable_bucket(cohort_key: &str, salt: &str) -> u8 {
    let mut hasher = Sha256::new();
    hasher.update(salt.as_bytes());
    hasher.update(b":");
    hasher.update(cohort_key.as_bytes());
    let digest = hasher.finalize();
    let digest_b64 = URL_SAFE_NO_PAD.encode(digest);
    let prefix = &digest_b64.as_bytes()[0..4];
    let mut value: u32 = 0;
    for byte in prefix {
        value = value.wrapping_mul(131).wrapping_add(*byte as u32);
    }
    (value % 100) as u8
}

fn is_codex_worker_control_path(path: &str) -> bool {
    path == "/api/runtime/codex/workers" || path.starts_with("/api/runtime/codex/workers/")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::SocketAddr;
    use std::path::PathBuf;

    fn test_config() -> Config {
        Config {
            bind_addr: SocketAddr::from(([127, 0, 0, 1], 0)),
            log_filter: "info".to_string(),
            static_dir: PathBuf::from("/tmp"),
            auth_provider_mode: "mock".to_string(),
            workos_client_id: None,
            workos_api_key: None,
            workos_api_base_url: "https://api.workos.com".to_string(),
            mock_magic_code: "123456".to_string(),
            auth_challenge_ttl_seconds: 600,
            auth_access_ttl_seconds: 3600,
            auth_refresh_ttl_seconds: 86400,
            sync_token_enabled: true,
            sync_token_signing_key: Some("sync-test-signing-key".to_string()),
            sync_token_issuer: "https://openagents.test".to_string(),
            sync_token_audience: "openagents-sync-test".to_string(),
            sync_token_key_id: "sync-auth-test-v1".to_string(),
            sync_token_claims_version: "oa_sync_claims_v1".to_string(),
            sync_token_ttl_seconds: 300,
            sync_token_min_ttl_seconds: 60,
            sync_token_max_ttl_seconds: 900,
            sync_token_allowed_scopes: vec!["runtime.codex_worker_events".to_string()],
            sync_token_default_scopes: vec!["runtime.codex_worker_events".to_string()],
            route_split_enabled: true,
            route_split_mode: "cohort".to_string(),
            route_split_rust_routes: vec![
                "/chat".to_string(),
                "/login".to_string(),
                "/register".to_string(),
                "/authenticate".to_string(),
                "/onboarding".to_string(),
                "/account".to_string(),
                "/settings".to_string(),
                "/l402".to_string(),
                "/billing".to_string(),
                "/admin".to_string(),
            ],
            route_split_cohort_percentage: 100,
            route_split_salt: "salt".to_string(),
            route_split_force_legacy: false,
            route_split_legacy_base_url: Some("https://legacy.example.com".to_string()),
            runtime_sync_revoke_base_url: None,
            runtime_sync_revoke_path: "/internal/v1/sync/sessions/revoke".to_string(),
            runtime_signature_secret: None,
            runtime_signature_ttl_seconds: 60,
            maintenance_mode_enabled: false,
            maintenance_bypass_token: None,
            maintenance_bypass_cookie_name: "oa_maintenance_bypass".to_string(),
            maintenance_bypass_cookie_ttl_seconds: 900,
            maintenance_allowed_paths: vec!["/healthz".to_string(), "/readyz".to_string()],
            compat_control_enforced: false,
            compat_control_protocol_version: "openagents.control.v1".to_string(),
            compat_control_min_client_build_id: "00000000T000000Z".to_string(),
            compat_control_max_client_build_id: None,
            compat_control_min_schema_version: 1,
            compat_control_max_schema_version: 1,
        }
    }

    #[tokio::test]
    async fn cohort_mode_targets_rust_when_bucket_is_in_range() {
        let service = RouteSplitService::from_config(&test_config());
        let decision = service.evaluate("/chat/thread-1", "user:1").await;

        assert_eq!(decision.target, RouteTarget::RustShell);
        assert_eq!(decision.reason, "mode_cohort");
        assert!(decision.cohort_bucket.is_some());
    }

    #[tokio::test]
    async fn runtime_override_forces_legacy_target() {
        let service = RouteSplitService::from_config(&test_config());
        service.set_override_target(Some(RouteTarget::Legacy)).await;

        let decision = service.evaluate("/chat", "user:1").await;
        assert_eq!(decision.target, RouteTarget::Legacy);
        assert_eq!(decision.reason, "runtime_override");
    }

    #[tokio::test]
    async fn management_prefixes_match_rust_routes() {
        let service = RouteSplitService::from_config(&test_config());

        for path in [
            "/account/session",
            "/settings/profile",
            "/l402/paywalls",
            "/billing/deployments",
            "/admin/tools",
        ] {
            let decision = service.evaluate(path, "user:1").await;
            assert_eq!(
                decision.target,
                RouteTarget::RustShell,
                "path should route to rust shell: {path}"
            );
        }
    }

    #[tokio::test]
    async fn auth_entry_prefixes_match_rust_routes() {
        let service = RouteSplitService::from_config(&test_config());

        for path in [
            "/login",
            "/register",
            "/authenticate",
            "/onboarding/checklist",
        ] {
            let decision = service.evaluate(path, "user:1").await;
            assert_eq!(
                decision.target,
                RouteTarget::RustShell,
                "path should route to rust shell: {path}"
            );
        }
    }

    #[tokio::test]
    async fn rust_mode_with_root_prefix_routes_all_paths_to_rust_shell() {
        let mut config = test_config();
        config.route_split_mode = "rust".to_string();
        config.route_split_rust_routes = vec!["/".to_string()];
        let service = RouteSplitService::from_config(&config);

        for path in ["/", "/login", "/settings/profile", "/unknown/path"] {
            let decision = service.evaluate(path, "user:1").await;
            assert_eq!(
                decision.target,
                RouteTarget::RustShell,
                "path should route to rust shell: {path}"
            );
            assert_eq!(decision.reason, "mode_rust");
        }
    }

    #[tokio::test]
    async fn codex_worker_control_paths_stay_on_legacy_even_in_rust_mode() {
        let mut config = test_config();
        config.route_split_mode = "rust".to_string();
        config.route_split_rust_routes = vec!["/".to_string()];
        let service = RouteSplitService::from_config(&config);

        for path in [
            "/api/runtime/codex/workers",
            "/api/runtime/codex/workers/codexw_1",
            "/api/runtime/codex/workers/codexw_1/events",
            "/api/runtime/codex/workers/codexw_1/requests",
            "/api/runtime/codex/workers/codexw_1/stop",
        ] {
            let decision = service.evaluate(path, "user:1").await;
            assert_eq!(
                decision.target,
                RouteTarget::Legacy,
                "codex worker control path must remain legacy: {path}"
            );
            assert_eq!(decision.reason, "codex_worker_control_legacy_authority");
        }
    }

    #[tokio::test]
    async fn codex_worker_control_paths_ignore_runtime_override_to_rust() {
        let service = RouteSplitService::from_config(&test_config());
        service
            .set_override_target(Some(RouteTarget::RustShell))
            .await;

        let decision = service
            .evaluate("/api/runtime/codex/workers/codexw_1/requests", "user:1")
            .await;

        assert_eq!(decision.target, RouteTarget::Legacy);
        assert_eq!(decision.reason, "codex_worker_control_legacy_authority");
    }
}
