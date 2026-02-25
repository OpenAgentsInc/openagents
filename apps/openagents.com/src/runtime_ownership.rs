use serde::Serialize;

use crate::openapi::{
    ROUTE_RUNTIME_CODEX_WORKER_BY_ID, ROUTE_RUNTIME_CODEX_WORKER_EVENTS,
    ROUTE_RUNTIME_CODEX_WORKER_REQUESTS, ROUTE_RUNTIME_CODEX_WORKER_STOP,
    ROUTE_RUNTIME_CODEX_WORKER_STREAM, ROUTE_RUNTIME_CODEX_WORKERS, ROUTE_RUNTIME_SKILLS_RELEASE,
    ROUTE_RUNTIME_SKILLS_SKILL_SPEC_PUBLISH, ROUTE_RUNTIME_SKILLS_SKILL_SPECS,
    ROUTE_RUNTIME_SKILLS_TOOL_SPECS, ROUTE_RUNTIME_THREAD_MESSAGES, ROUTE_RUNTIME_THREADS,
    ROUTE_RUNTIME_TOOLS_EXECUTE, ROUTE_RUNTIME_WORKER_BY_ID, ROUTE_RUNTIME_WORKER_HEARTBEAT,
    ROUTE_RUNTIME_WORKER_STATUS, ROUTE_RUNTIME_WORKERS,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeRouteOwner {
    ControlService,
    RuntimeService,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeRouteDelivery {
    InProcess,
    RuntimeProxy,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct RuntimeRouteOwnership {
    pub method: &'static str,
    pub path: &'static str,
    pub owner: RuntimeRouteOwner,
    pub delivery: RuntimeRouteDelivery,
    pub migration_status: &'static str,
    pub rationale: &'static str,
}

const fn control_owned(
    method: &'static str,
    path: &'static str,
    rationale: &'static str,
) -> RuntimeRouteOwnership {
    RuntimeRouteOwnership {
        method,
        path,
        owner: RuntimeRouteOwner::ControlService,
        delivery: RuntimeRouteDelivery::InProcess,
        migration_status: "control_native",
        rationale,
    }
}

const fn runtime_proxy(
    method: &'static str,
    path: &'static str,
    rationale: &'static str,
) -> RuntimeRouteOwnership {
    RuntimeRouteOwnership {
        method,
        path,
        owner: RuntimeRouteOwner::RuntimeService,
        delivery: RuntimeRouteDelivery::RuntimeProxy,
        migration_status: "runtime_authority_proxy",
        rationale,
    }
}

pub const RUNTIME_ROUTE_OWNERSHIP: &[RuntimeRouteOwnership] = &[
    control_owned(
        "POST",
        ROUTE_RUNTIME_TOOLS_EXECUTE,
        "policy receipts and tool execution envelopes are control-owned in this lane",
    ),
    control_owned(
        "GET",
        ROUTE_RUNTIME_SKILLS_TOOL_SPECS,
        "skill catalog reads are served by control-owned registry projections",
    ),
    control_owned(
        "POST",
        ROUTE_RUNTIME_SKILLS_TOOL_SPECS,
        "skill tool-spec writes remain in control-owned registry lane",
    ),
    control_owned(
        "GET",
        ROUTE_RUNTIME_SKILLS_SKILL_SPECS,
        "skill-spec reads are served by control-owned registry projections",
    ),
    control_owned(
        "POST",
        ROUTE_RUNTIME_SKILLS_SKILL_SPECS,
        "skill-spec writes remain in control-owned registry lane",
    ),
    control_owned(
        "POST",
        ROUTE_RUNTIME_SKILLS_SKILL_SPEC_PUBLISH,
        "skill release publication is handled by control-owned registry lane",
    ),
    control_owned(
        "GET",
        ROUTE_RUNTIME_SKILLS_RELEASE,
        "published skill bundle readback is served from control-owned registry lane",
    ),
    control_owned(
        "GET",
        ROUTE_RUNTIME_THREADS,
        "thread projection read API is control-owned for current codex projection store",
    ),
    control_owned(
        "GET",
        ROUTE_RUNTIME_THREAD_MESSAGES,
        "thread message projection read API is control-owned for codex projection store",
    ),
    control_owned(
        "POST",
        ROUTE_RUNTIME_THREAD_MESSAGES,
        "thread command ingestion for codex worker lane is control-owned",
    ),
    control_owned(
        "GET",
        ROUTE_RUNTIME_CODEX_WORKERS,
        "desktop codex worker session directory is control-owned in this lane",
    ),
    control_owned(
        "POST",
        ROUTE_RUNTIME_CODEX_WORKERS,
        "desktop codex worker session creation is control-owned in this lane",
    ),
    control_owned(
        "GET",
        ROUTE_RUNTIME_CODEX_WORKER_BY_ID,
        "desktop codex worker snapshot read is control-owned in this lane",
    ),
    control_owned(
        "GET",
        ROUTE_RUNTIME_CODEX_WORKER_STREAM,
        "codex worker stream bootstrap and replay cursor lane is control-owned",
    ),
    control_owned(
        "POST",
        ROUTE_RUNTIME_CODEX_WORKER_EVENTS,
        "desktop-originated codex worker events are ingested in control lane",
    ),
    control_owned(
        "POST",
        ROUTE_RUNTIME_CODEX_WORKER_STOP,
        "codex worker terminal state transitions are control-owned in this lane",
    ),
    control_owned(
        "POST",
        ROUTE_RUNTIME_CODEX_WORKER_REQUESTS,
        "allowlisted codex control requests are mediated in control lane",
    ),
    runtime_proxy(
        "GET",
        ROUTE_RUNTIME_WORKERS,
        "runtime worker authority list is proxied to runtime internal authority APIs",
    ),
    runtime_proxy(
        "POST",
        ROUTE_RUNTIME_WORKERS,
        "runtime worker authority registration is proxied to runtime internal APIs",
    ),
    runtime_proxy(
        "GET",
        ROUTE_RUNTIME_WORKER_BY_ID,
        "runtime worker authority read is proxied to runtime internal APIs",
    ),
    runtime_proxy(
        "POST",
        ROUTE_RUNTIME_WORKER_HEARTBEAT,
        "runtime worker heartbeat authority update is proxied to runtime internal APIs",
    ),
    runtime_proxy(
        "POST",
        ROUTE_RUNTIME_WORKER_STATUS,
        "runtime worker status authority update is proxied to runtime internal APIs",
    ),
];

#[must_use]
pub fn runtime_route_ownership() -> &'static [RuntimeRouteOwnership] {
    RUNTIME_ROUTE_OWNERSHIP
}

#[must_use]
pub fn runtime_route_owner(method: &str, path: &str) -> Option<RuntimeRouteOwner> {
    let normalized_method = method.trim().to_ascii_uppercase();
    RUNTIME_ROUTE_OWNERSHIP
        .iter()
        .find(|entry| entry.method == normalized_method && entry.path == path)
        .map(|entry| entry.owner)
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::*;

    #[test]
    fn runtime_ownership_contract_is_unambiguous() {
        let mut seen = HashSet::new();
        for entry in runtime_route_ownership() {
            let key = format!("{} {}", entry.method, entry.path);
            assert!(
                seen.insert(key.clone()),
                "duplicate runtime ownership entry: {key}"
            );
        }
    }

    #[test]
    fn runtime_ownership_contract_covers_known_runtime_routes() {
        let required = [
            ("POST", ROUTE_RUNTIME_TOOLS_EXECUTE),
            ("GET", ROUTE_RUNTIME_SKILLS_TOOL_SPECS),
            ("POST", ROUTE_RUNTIME_SKILLS_TOOL_SPECS),
            ("GET", ROUTE_RUNTIME_SKILLS_SKILL_SPECS),
            ("POST", ROUTE_RUNTIME_SKILLS_SKILL_SPECS),
            ("POST", ROUTE_RUNTIME_SKILLS_SKILL_SPEC_PUBLISH),
            ("GET", ROUTE_RUNTIME_SKILLS_RELEASE),
            ("GET", ROUTE_RUNTIME_THREADS),
            ("GET", ROUTE_RUNTIME_THREAD_MESSAGES),
            ("POST", ROUTE_RUNTIME_THREAD_MESSAGES),
            ("GET", ROUTE_RUNTIME_CODEX_WORKERS),
            ("POST", ROUTE_RUNTIME_CODEX_WORKERS),
            ("GET", ROUTE_RUNTIME_CODEX_WORKER_BY_ID),
            ("GET", ROUTE_RUNTIME_CODEX_WORKER_STREAM),
            ("POST", ROUTE_RUNTIME_CODEX_WORKER_EVENTS),
            ("POST", ROUTE_RUNTIME_CODEX_WORKER_STOP),
            ("POST", ROUTE_RUNTIME_CODEX_WORKER_REQUESTS),
            ("GET", ROUTE_RUNTIME_WORKERS),
            ("POST", ROUTE_RUNTIME_WORKERS),
            ("GET", ROUTE_RUNTIME_WORKER_BY_ID),
            ("POST", ROUTE_RUNTIME_WORKER_HEARTBEAT),
            ("POST", ROUTE_RUNTIME_WORKER_STATUS),
        ];

        for (method, path) in required {
            assert!(
                runtime_route_owner(method, path).is_some(),
                "missing runtime ownership entry for {} {}",
                method,
                path
            );
        }
    }

    #[test]
    fn runtime_service_routes_are_runtime_proxy_delivery() {
        for entry in runtime_route_ownership() {
            if entry.owner == RuntimeRouteOwner::RuntimeService {
                assert_eq!(
                    entry.delivery,
                    RuntimeRouteDelivery::RuntimeProxy,
                    "runtime-owned route is not documented as runtime proxy: {} {}",
                    entry.method,
                    entry.path
                );
                assert_eq!(entry.migration_status, "runtime_authority_proxy");
            }
        }
    }
}
