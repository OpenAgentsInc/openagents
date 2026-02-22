use std::collections::HashMap;
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
    pub route_domain: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rollback_target: Option<RouteTarget>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cohort_bucket: Option<u8>,
    pub cohort_key: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct RouteGroupStatus {
    pub domain: String,
    pub route_prefixes: Vec<String>,
    pub rollback_target: RouteTarget,
    pub override_target: Option<RouteTarget>,
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
    pub route_groups: Vec<RouteGroupStatus>,
    pub rollback_matrix: HashMap<String, RouteTarget>,
    pub domain_overrides: HashMap<String, RouteTarget>,
}

#[derive(Clone)]
pub struct RouteSplitService {
    config: RouteSplitConfig,
    override_target: Arc<RwLock<Option<RouteTarget>>>,
    domain_overrides: Arc<RwLock<HashMap<String, RouteTarget>>>,
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
    route_groups: Vec<RouteGroupConfig>,
    rollback_matrix: HashMap<String, RouteTarget>,
}

#[derive(Debug, Clone)]
struct RouteGroupConfig {
    domain: String,
    route_prefixes: Vec<String>,
    rollback_target: RouteTarget,
}

impl RouteSplitService {
    pub fn from_config(config: &Config) -> Self {
        Self {
            config: RouteSplitConfig::from_config(config),
            override_target: Arc::new(RwLock::new(None)),
            domain_overrides: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn evaluate(&self, path: &str, cohort_key: &str) -> RouteSplitDecision {
        let normalized_path = normalize_path(path);
        let normalized_cohort_key = normalize_cohort_key(cohort_key);
        let route_domain = self.route_domain_for_path(&normalized_path);
        let rollback_target = self.rollback_target_for_domain(route_domain.as_deref());
        let override_target = *self.override_target.read().await;

        // Codex worker control APIs are Rust authority and must not route to legacy.
        if is_codex_worker_control_path(&normalized_path) {
            return RouteSplitDecision {
                path: normalized_path,
                target: RouteTarget::RustShell,
                reason: "codex_worker_control_rust_authority".to_string(),
                route_domain: "runtime_codex_worker_control".to_string(),
                rollback_target: Some(RouteTarget::RustShell),
                cohort_bucket: None,
                cohort_key: normalized_cohort_key,
            };
        }

        // All API paths are Rust authority and must never route to legacy.
        if is_api_path(&normalized_path) {
            return RouteSplitDecision {
                path: normalized_path,
                target: RouteTarget::RustShell,
                reason: "api_rust_authority".to_string(),
                route_domain: "api_rust_authority".to_string(),
                rollback_target: Some(RouteTarget::RustShell),
                cohort_bucket: None,
                cohort_key: normalized_cohort_key,
            };
        }

        let domain_override = {
            let lock = self.domain_overrides.read().await;
            route_domain
                .as_ref()
                .and_then(|domain| lock.get(domain).copied())
        };

        if let Some(target) = domain_override {
            return RouteSplitDecision {
                path: normalized_path,
                target,
                reason: "domain_override".to_string(),
                route_domain: route_domain.unwrap_or_else(|| "unclassified".to_string()),
                rollback_target,
                cohort_bucket: None,
                cohort_key: normalized_cohort_key,
            };
        }

        if self.config.force_legacy {
            return RouteSplitDecision {
                path: normalized_path,
                target: RouteTarget::Legacy,
                reason: "force_legacy".to_string(),
                route_domain: route_domain.unwrap_or_else(|| "unclassified".to_string()),
                rollback_target,
                cohort_bucket: None,
                cohort_key: normalized_cohort_key,
            };
        }

        if let Some(target) = override_target {
            return RouteSplitDecision {
                path: normalized_path,
                target,
                reason: "runtime_override".to_string(),
                route_domain: route_domain.unwrap_or_else(|| "unclassified".to_string()),
                rollback_target,
                cohort_bucket: None,
                cohort_key: normalized_cohort_key,
            };
        }

        if !self.config.enabled {
            return RouteSplitDecision {
                path: normalized_path,
                target: RouteTarget::Legacy,
                reason: "route_split_disabled".to_string(),
                route_domain: route_domain.unwrap_or_else(|| "unclassified".to_string()),
                rollback_target,
                cohort_bucket: None,
                cohort_key: normalized_cohort_key,
            };
        }

        if !self.matches_rust_route(path) {
            return RouteSplitDecision {
                path: normalized_path,
                target: RouteTarget::Legacy,
                reason: "legacy_route_default".to_string(),
                route_domain: route_domain.unwrap_or_else(|| "unclassified".to_string()),
                rollback_target,
                cohort_bucket: None,
                cohort_key: normalized_cohort_key,
            };
        }

        match self.config.mode {
            RouteMode::Legacy => RouteSplitDecision {
                path: normalized_path,
                target: RouteTarget::Legacy,
                reason: "mode_legacy".to_string(),
                route_domain: route_domain.unwrap_or_else(|| "unclassified".to_string()),
                rollback_target,
                cohort_bucket: None,
                cohort_key: normalized_cohort_key,
            },
            RouteMode::Rust => RouteSplitDecision {
                path: normalized_path,
                target: RouteTarget::RustShell,
                reason: "mode_rust".to_string(),
                route_domain: route_domain.unwrap_or_else(|| "unclassified".to_string()),
                rollback_target,
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
                    route_domain: route_domain.unwrap_or_else(|| "unclassified".to_string()),
                    rollback_target,
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

    pub async fn set_domain_override_target(
        &self,
        domain: &str,
        target: Option<RouteTarget>,
    ) -> Result<(), String> {
        let normalized = normalize_domain(domain);
        if !self.config.rollback_matrix.contains_key(&normalized) {
            return Err(format!("Unknown route domain '{domain}'."));
        }

        let mut lock = self.domain_overrides.write().await;
        if let Some(target) = target {
            lock.insert(normalized, target);
        } else {
            lock.remove(&normalized);
        }
        Ok(())
    }

    pub fn rollback_target_for_domain(&self, domain: Option<&str>) -> Option<RouteTarget> {
        let Some(domain) = domain else {
            return Some(RouteTarget::Legacy);
        };

        let normalized = normalize_domain(domain);
        self.config.rollback_matrix.get(&normalized).copied()
    }

    pub async fn status(&self) -> RouteSplitStatus {
        let override_target = *self.override_target.read().await;
        let domain_overrides = self.domain_overrides.read().await.clone();
        let route_groups = self
            .config
            .route_groups
            .iter()
            .map(|group| RouteGroupStatus {
                domain: group.domain.clone(),
                route_prefixes: group.route_prefixes.clone(),
                rollback_target: group.rollback_target,
                override_target: domain_overrides.get(&group.domain).copied(),
            })
            .collect();

        RouteSplitStatus {
            enabled: self.config.enabled,
            mode: self.config.mode.as_str().to_string(),
            cohort_percentage: self.config.cohort_percentage,
            rust_routes: self.config.rust_routes.clone(),
            force_legacy: self.config.force_legacy,
            legacy_base_url: self.config.legacy_base_url.clone(),
            override_target,
            route_groups,
            rollback_matrix: self.config.rollback_matrix.clone(),
            domain_overrides,
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

    fn route_domain_for_path(&self, path: &str) -> Option<String> {
        let normalized = normalize_path(path);

        self.config.route_groups.iter().find_map(|group| {
            if group.route_prefixes.iter().any(|prefix| {
                if prefix == "/" {
                    return normalized == "/";
                }
                normalized == *prefix || normalized.starts_with(&format!("{prefix}/"))
            }) {
                return Some(group.domain.clone());
            }
            None
        })
    }
}

impl RouteSplitConfig {
    fn from_config(config: &Config) -> Self {
        let route_groups = default_route_groups();
        let rollback_matrix = route_groups
            .iter()
            .map(|group| (group.domain.clone(), group.rollback_target))
            .collect();

        Self {
            enabled: config.route_split_enabled,
            mode: RouteMode::from_str(&config.route_split_mode),
            rust_routes: normalize_routes(&config.route_split_rust_routes),
            cohort_percentage: config.route_split_cohort_percentage.min(100),
            salt: config.route_split_salt.clone(),
            force_legacy: config.route_split_force_legacy,
            legacy_base_url: config.route_split_legacy_base_url.clone(),
            route_groups,
            rollback_matrix,
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

fn is_api_path(path: &str) -> bool {
    path == "/api" || path.starts_with("/api/")
}

fn normalize_domain(value: &str) -> String {
    value.trim().to_lowercase().replace('-', "_")
}

fn default_route_groups() -> Vec<RouteGroupConfig> {
    vec![
        RouteGroupConfig {
            domain: "auth_entry".to_string(),
            route_prefixes: vec![
                "/login".to_string(),
                "/register".to_string(),
                "/authenticate".to_string(),
                "/onboarding".to_string(),
            ],
            rollback_target: RouteTarget::Legacy,
        },
        RouteGroupConfig {
            domain: "account_settings_admin".to_string(),
            route_prefixes: vec![
                "/account".to_string(),
                "/settings".to_string(),
                "/admin".to_string(),
            ],
            rollback_target: RouteTarget::Legacy,
        },
        RouteGroupConfig {
            domain: "billing_l402".to_string(),
            route_prefixes: vec!["/billing".to_string(), "/l402".to_string()],
            rollback_target: RouteTarget::Legacy,
        },
        RouteGroupConfig {
            domain: "chat_pilot".to_string(),
            route_prefixes: vec!["/chat".to_string(), "/feed".to_string(), "/".to_string()],
            rollback_target: RouteTarget::RustShell,
        },
    ]
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
            auth_local_test_login_enabled: false,
            auth_local_test_login_allowed_emails: vec![],
            auth_local_test_login_signing_key: None,
            auth_api_signup_enabled: false,
            auth_api_signup_allowed_domains: vec![],
            auth_api_signup_default_token_name: "api-bootstrap".to_string(),
            admin_emails: vec![],
            khala_token_enabled: true,
            khala_token_signing_key: Some("khala-test-signing-key".to_string()),
            khala_token_issuer: "https://openagents.test".to_string(),
            khala_token_audience: "openagents-khala-test".to_string(),
            khala_token_subject_prefix: "user".to_string(),
            khala_token_key_id: "khala-auth-test-v1".to_string(),
            khala_token_claims_version: "oa_khala_claims_v1".to_string(),
            khala_token_ttl_seconds: 300,
            khala_token_min_ttl_seconds: 60,
            khala_token_max_ttl_seconds: 900,
            auth_store_path: None,
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
            runtime_internal_shared_secret: None,
            runtime_internal_key_id: "runtime-internal-v1".to_string(),
            runtime_internal_signature_ttl_seconds: 60,
            runtime_internal_secret_fetch_path: "/api/internal/runtime/integrations/secrets/fetch"
                .to_string(),
            runtime_internal_secret_cache_ttl_ms: 60_000,
            runtime_elixir_base_url: None,
            runtime_signing_key: None,
            runtime_signing_key_id: "runtime-v1".to_string(),
            runtime_comms_delivery_ingest_path: "/internal/v1/comms/delivery-events".to_string(),
            runtime_comms_delivery_timeout_ms: 10_000,
            runtime_comms_delivery_max_retries: 2,
            runtime_comms_delivery_retry_backoff_ms: 200,
            smoke_stream_secret: None,
            resend_webhook_secret: None,
            resend_webhook_tolerance_seconds: 300,
            google_oauth_client_id: None,
            google_oauth_client_secret: None,
            google_oauth_redirect_uri: None,
            google_oauth_scopes: "https://www.googleapis.com/auth/gmail.readonly".to_string(),
            google_oauth_token_url: "https://oauth2.googleapis.com/token".to_string(),
            runtime_driver: "legacy".to_string(),
            runtime_force_driver: None,
            runtime_force_legacy: false,
            runtime_canary_user_percent: 0,
            runtime_canary_autopilot_percent: 0,
            runtime_canary_seed: "runtime-canary-v1".to_string(),
            runtime_overrides_enabled: true,
            runtime_shadow_enabled: false,
            runtime_shadow_sample_rate: 1.0,
            runtime_shadow_max_capture_bytes: 200_000,
            codex_thread_store_path: None,
            domain_store_path: None,
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
    async fn codex_worker_control_paths_stay_on_rust_even_with_legacy_biased_modes() {
        let mut config = test_config();
        config.route_split_mode = "legacy".to_string();
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
                RouteTarget::RustShell,
                "codex worker control path must remain rust authority: {path}"
            );
            assert_eq!(decision.reason, "codex_worker_control_rust_authority");
        }
    }

    #[tokio::test]
    async fn codex_worker_control_paths_ignore_runtime_override_to_legacy() {
        let service = RouteSplitService::from_config(&test_config());
        service.set_override_target(Some(RouteTarget::Legacy)).await;

        let decision = service
            .evaluate("/api/runtime/codex/workers/codexw_1/requests", "user:1")
            .await;

        assert_eq!(decision.target, RouteTarget::RustShell);
        assert_eq!(decision.reason, "codex_worker_control_rust_authority");
    }

    #[tokio::test]
    async fn codex_worker_control_paths_ignore_force_legacy() {
        let mut config = test_config();
        config.route_split_force_legacy = true;
        let service = RouteSplitService::from_config(&config);

        let decision = service
            .evaluate("/api/runtime/codex/workers/codexw_1/requests", "user:1")
            .await;

        assert_eq!(decision.target, RouteTarget::RustShell);
        assert_eq!(decision.reason, "codex_worker_control_rust_authority");
    }

    #[tokio::test]
    async fn api_paths_are_rust_authority_even_under_legacy_overrides() {
        let mut config = test_config();
        config.route_split_mode = "legacy".to_string();
        config.route_split_force_legacy = true;
        let service = RouteSplitService::from_config(&config);
        service.set_override_target(Some(RouteTarget::Legacy)).await;
        service
            .set_domain_override_target("billing_l402", Some(RouteTarget::Legacy))
            .await
            .expect("set domain override");

        for path in [
            "/api/auth/email",
            "/api/settings/profile",
            "/api/runtime/tools/execute",
            "/api/v1/control/status",
        ] {
            let decision = service.evaluate(path, "user:1").await;
            assert_eq!(
                decision.target,
                RouteTarget::RustShell,
                "api path must remain rust authority: {path}"
            );
            assert_eq!(decision.reason, "api_rust_authority");
            assert_eq!(decision.route_domain, "api_rust_authority");
            assert_eq!(decision.rollback_target, Some(RouteTarget::RustShell));
        }
    }

    #[tokio::test]
    async fn domain_override_applies_only_to_matching_route_group() {
        let service = RouteSplitService::from_config(&test_config());
        service
            .set_domain_override_target("billing_l402", Some(RouteTarget::Legacy))
            .await
            .expect("set domain override");

        let billing_decision = service.evaluate("/l402/paywalls", "user:1").await;
        assert_eq!(billing_decision.target, RouteTarget::Legacy);
        assert_eq!(billing_decision.reason, "domain_override");
        assert_eq!(billing_decision.route_domain, "billing_l402");

        let settings_decision = service.evaluate("/settings/profile", "user:1").await;
        assert_eq!(settings_decision.target, RouteTarget::RustShell);
        assert_ne!(settings_decision.reason, "domain_override");
    }

    #[tokio::test]
    async fn status_exposes_rollback_matrix_per_route_group() {
        let service = RouteSplitService::from_config(&test_config());
        let status = service.status().await;

        assert_eq!(
            status.rollback_matrix.get("auth_entry"),
            Some(&RouteTarget::Legacy)
        );
        assert_eq!(
            status.rollback_matrix.get("account_settings_admin"),
            Some(&RouteTarget::Legacy)
        );
        assert_eq!(
            status.rollback_matrix.get("billing_l402"),
            Some(&RouteTarget::Legacy)
        );
        assert_eq!(
            status.rollback_matrix.get("chat_pilot"),
            Some(&RouteTarget::RustShell)
        );
    }

    #[tokio::test]
    async fn route_decision_carries_domain_and_rollback_target() {
        let service = RouteSplitService::from_config(&test_config());
        let decision = service.evaluate("/settings/profile", "user:1").await;

        assert_eq!(decision.route_domain, "account_settings_admin");
        assert_eq!(decision.rollback_target, Some(RouteTarget::Legacy));
    }
}
