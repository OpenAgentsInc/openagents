use crate::auth::{
    AuthInputError, DEFAULT_CONTROL_BASE_URL, normalize_base_url, resolve_control_base_url,
};

pub const ENV_RUNTIME_SYNC_BASE_URL: &str = "OPENAGENTS_RUNTIME_SYNC_BASE_URL";
pub const ENV_RUNTIME_BASE_URL: &str = "OPENAGENTS_RUNTIME_BASE_URL";
pub const ENV_EXECUTION_FALLBACK_ORDER: &str = "OPENAGENTS_EXECUTION_FALLBACK_ORDER";
pub const RUNTIME_BASE_SOURCE_STORED_AUTH: &str = "runtime_auth_state";
pub const RUNTIME_BASE_SOURCE_DEFAULT_LOCAL: &str = "default_local";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedRuntimeBaseUrl {
    pub base_url: String,
    pub source: String,
    pub locked_by_env: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExecutionFallbackOrder {
    LocalOnly,
    LocalThenRuntime,
    LocalThenRuntimeThenSwarm,
}

impl ExecutionFallbackOrder {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::LocalOnly => "local_only",
            Self::LocalThenRuntime => "local_then_runtime",
            Self::LocalThenRuntimeThenSwarm => "local_then_runtime_then_swarm",
        }
    }

    #[must_use]
    pub fn allows_runtime(self) -> bool {
        !matches!(self, Self::LocalOnly)
    }

    #[must_use]
    pub fn allows_swarm(self) -> bool {
        matches!(self, Self::LocalThenRuntimeThenSwarm)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExecutionLane {
    LocalCodex,
    SharedRuntime,
    Swarm,
}

impl ExecutionLane {
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::LocalCodex => "local_codex",
            Self::SharedRuntime => "shared_runtime",
            Self::Swarm => "swarm",
        }
    }
}

const EXECUTION_LANE_ORDER_LOCAL_ONLY: [ExecutionLane; 1] = [ExecutionLane::LocalCodex];
const EXECUTION_LANE_ORDER_LOCAL_RUNTIME: [ExecutionLane; 2] =
    [ExecutionLane::LocalCodex, ExecutionLane::SharedRuntime];
const EXECUTION_LANE_ORDER_LOCAL_RUNTIME_SWARM: [ExecutionLane; 3] = [
    ExecutionLane::LocalCodex,
    ExecutionLane::SharedRuntime,
    ExecutionLane::Swarm,
];

#[must_use]
pub fn execution_lane_order(order: ExecutionFallbackOrder) -> &'static [ExecutionLane] {
    match order {
        ExecutionFallbackOrder::LocalOnly => &EXECUTION_LANE_ORDER_LOCAL_ONLY,
        ExecutionFallbackOrder::LocalThenRuntime => &EXECUTION_LANE_ORDER_LOCAL_RUNTIME,
        ExecutionFallbackOrder::LocalThenRuntimeThenSwarm => {
            &EXECUTION_LANE_ORDER_LOCAL_RUNTIME_SWARM
        }
    }
}

#[must_use]
pub fn parse_execution_fallback_order(raw: &str) -> Option<ExecutionFallbackOrder> {
    let normalized = raw.trim().to_ascii_lowercase();
    match normalized.as_str() {
        "local_only" | "local-only" | "local" => Some(ExecutionFallbackOrder::LocalOnly),
        "local_then_runtime" | "local-then-runtime" | "local_runtime" => {
            Some(ExecutionFallbackOrder::LocalThenRuntime)
        }
        "local_then_runtime_then_swarm"
        | "local-then-runtime-then-swarm"
        | "local_runtime_swarm"
        | "local_runtime_network" => Some(ExecutionFallbackOrder::LocalThenRuntimeThenSwarm),
        _ => None,
    }
}

#[must_use]
pub fn resolve_execution_fallback_order() -> (ExecutionFallbackOrder, String) {
    if let Some(raw) = env_non_empty(ENV_EXECUTION_FALLBACK_ORDER) {
        if let Some(order) = parse_execution_fallback_order(&raw) {
            return (order, ENV_EXECUTION_FALLBACK_ORDER.to_string());
        }
        return (
            ExecutionFallbackOrder::LocalThenRuntimeThenSwarm,
            format!(
                "{}:invalid({raw})->{}",
                ENV_EXECUTION_FALLBACK_ORDER,
                ExecutionFallbackOrder::LocalThenRuntimeThenSwarm.as_str()
            ),
        );
    }

    (
        ExecutionFallbackOrder::LocalThenRuntimeThenSwarm,
        "default".to_string(),
    )
}

pub fn resolve_runtime_sync_base_url(
    stored_auth_base_url: Option<&str>,
) -> Result<ResolvedRuntimeBaseUrl, AuthInputError> {
    if let Some(base_url) = env_non_empty(ENV_RUNTIME_SYNC_BASE_URL) {
        return Ok(ResolvedRuntimeBaseUrl {
            base_url: normalize_base_url(&base_url)?,
            source: ENV_RUNTIME_SYNC_BASE_URL.to_string(),
            locked_by_env: true,
        });
    }

    if let Some(base_url) = env_non_empty(ENV_RUNTIME_BASE_URL) {
        return Ok(ResolvedRuntimeBaseUrl {
            base_url: normalize_base_url(&base_url)?,
            source: ENV_RUNTIME_BASE_URL.to_string(),
            locked_by_env: true,
        });
    }

    let (control_base_url, control_source) = resolve_control_base_url()?;
    if control_source != RUNTIME_BASE_SOURCE_DEFAULT_LOCAL {
        return Ok(ResolvedRuntimeBaseUrl {
            base_url: control_base_url,
            source: control_source.to_string(),
            locked_by_env: true,
        });
    }

    if let Some(base_url) = stored_auth_base_url
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
    {
        return Ok(ResolvedRuntimeBaseUrl {
            base_url: normalize_base_url(base_url)?,
            source: RUNTIME_BASE_SOURCE_STORED_AUTH.to_string(),
            locked_by_env: false,
        });
    }

    Ok(ResolvedRuntimeBaseUrl {
        base_url: normalize_base_url(DEFAULT_CONTROL_BASE_URL)?,
        source: RUNTIME_BASE_SOURCE_DEFAULT_LOCAL.to_string(),
        locked_by_env: false,
    })
}

fn env_non_empty(key: &str) -> Option<String> {
    std::env::var(key)
        .ok()
        .map(|value| value.trim().trim_end_matches('/').to_string())
        .filter(|value| !value.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::{ENV_CONTROL_BASE_URL, ENV_CONTROL_BASE_URL_LEGACY};
    use std::sync::{Mutex, OnceLock};

    static ENV_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

    fn with_env<T>(overrides: &[(&str, Option<&str>)], test: impl FnOnce() -> T) -> T {
        let lock = ENV_LOCK.get_or_init(|| Mutex::new(()));
        let _guard = lock.lock().unwrap_or_else(|poisoned| poisoned.into_inner());

        let previous = overrides
            .iter()
            .map(|(key, _)| (*key, std::env::var(key).ok()))
            .collect::<Vec<_>>();

        for (key, value) in overrides {
            if let Some(value) = value {
                unsafe { std::env::set_var(key, value) };
            } else {
                unsafe { std::env::remove_var(key) };
            }
        }

        let result = test();

        for (key, value) in previous {
            if let Some(value) = value {
                unsafe { std::env::set_var(key, value) };
            } else {
                unsafe { std::env::remove_var(key) };
            }
        }

        result
    }

    #[test]
    fn runtime_sync_base_prefers_runtime_specific_env() {
        with_env(
            &[
                (
                    ENV_RUNTIME_SYNC_BASE_URL,
                    Some("https://runtime.staging.openagents.com/"),
                ),
                (ENV_RUNTIME_BASE_URL, Some("https://runtime.example.com")),
                (
                    ENV_CONTROL_BASE_URL,
                    Some("https://control.staging.openagents.com"),
                ),
                (
                    ENV_CONTROL_BASE_URL_LEGACY,
                    Some("https://legacy.example.com"),
                ),
            ],
            || {
                let resolved = resolve_runtime_sync_base_url(None).expect("resolved");
                assert_eq!(resolved.base_url, "https://runtime.staging.openagents.com");
                assert_eq!(resolved.source, ENV_RUNTIME_SYNC_BASE_URL);
                assert!(resolved.locked_by_env);
            },
        );
    }

    #[test]
    fn runtime_sync_base_uses_stored_auth_when_no_env_overrides() {
        with_env(
            &[
                (ENV_RUNTIME_SYNC_BASE_URL, None),
                (ENV_RUNTIME_BASE_URL, None),
                (ENV_CONTROL_BASE_URL, None),
                (ENV_CONTROL_BASE_URL_LEGACY, None),
            ],
            || {
                let resolved = resolve_runtime_sync_base_url(Some("https://saved.example.com/"))
                    .expect("resolved");
                assert_eq!(resolved.base_url, "https://saved.example.com");
                assert_eq!(resolved.source, RUNTIME_BASE_SOURCE_STORED_AUTH);
                assert!(!resolved.locked_by_env);
            },
        );
    }

    #[test]
    fn runtime_sync_base_defaults_local_when_no_inputs() {
        with_env(
            &[
                (ENV_RUNTIME_SYNC_BASE_URL, None),
                (ENV_RUNTIME_BASE_URL, None),
                (ENV_CONTROL_BASE_URL, None),
                (ENV_CONTROL_BASE_URL_LEGACY, None),
            ],
            || {
                let resolved = resolve_runtime_sync_base_url(None).expect("resolved");
                assert_eq!(resolved.base_url, DEFAULT_CONTROL_BASE_URL);
                assert_eq!(resolved.source, RUNTIME_BASE_SOURCE_DEFAULT_LOCAL);
                assert!(!resolved.locked_by_env);
            },
        );
    }

    #[test]
    fn execution_fallback_order_defaults_to_runtime_and_swarm() {
        with_env(&[(ENV_EXECUTION_FALLBACK_ORDER, None)], || {
            let (order, source) = resolve_execution_fallback_order();
            assert_eq!(order, ExecutionFallbackOrder::LocalThenRuntimeThenSwarm);
            assert_eq!(source, "default");
        });
    }

    #[test]
    fn execution_fallback_order_respects_env_override() {
        with_env(
            &[(ENV_EXECUTION_FALLBACK_ORDER, Some("local_then_runtime"))],
            || {
                let (order, source) = resolve_execution_fallback_order();
                assert_eq!(order, ExecutionFallbackOrder::LocalThenRuntime);
                assert_eq!(source, ENV_EXECUTION_FALLBACK_ORDER);
            },
        );
    }

    #[test]
    fn execution_lane_order_keeps_local_codex_first_for_all_policies() {
        for order in [
            ExecutionFallbackOrder::LocalOnly,
            ExecutionFallbackOrder::LocalThenRuntime,
            ExecutionFallbackOrder::LocalThenRuntimeThenSwarm,
        ] {
            let lanes = execution_lane_order(order);
            assert_eq!(lanes.first(), Some(&ExecutionLane::LocalCodex));
        }
    }

    #[test]
    fn execution_lane_order_matches_policy_shape() {
        assert_eq!(
            execution_lane_order(ExecutionFallbackOrder::LocalOnly),
            [ExecutionLane::LocalCodex]
        );
        assert_eq!(
            execution_lane_order(ExecutionFallbackOrder::LocalThenRuntime),
            [ExecutionLane::LocalCodex, ExecutionLane::SharedRuntime]
        );
        assert_eq!(
            execution_lane_order(ExecutionFallbackOrder::LocalThenRuntimeThenSwarm),
            [
                ExecutionLane::LocalCodex,
                ExecutionLane::SharedRuntime,
                ExecutionLane::Swarm,
            ]
        );
    }
}
