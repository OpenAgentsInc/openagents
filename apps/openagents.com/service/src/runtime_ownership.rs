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
pub struct RuntimeRouteOwnership {
    pub method: &'static str,
    pub path: &'static str,
    pub owner: RuntimeRouteOwner,
    pub rationale: &'static str,
}

pub const RUNTIME_ROUTE_OWNERSHIP: &[RuntimeRouteOwnership] = &[
    RuntimeRouteOwnership {
        method: "POST",
        path: ROUTE_RUNTIME_TOOLS_EXECUTE,
        owner: RuntimeRouteOwner::ControlService,
        rationale: "policy receipts and tool execution envelopes are control-owned in this lane",
    },
    RuntimeRouteOwnership {
        method: "GET",
        path: ROUTE_RUNTIME_SKILLS_TOOL_SPECS,
        owner: RuntimeRouteOwner::ControlService,
        rationale: "skill catalog reads are served by control-owned registry projections",
    },
    RuntimeRouteOwnership {
        method: "POST",
        path: ROUTE_RUNTIME_SKILLS_TOOL_SPECS,
        owner: RuntimeRouteOwner::ControlService,
        rationale: "skill tool-spec writes remain in control-owned registry lane",
    },
    RuntimeRouteOwnership {
        method: "GET",
        path: ROUTE_RUNTIME_SKILLS_SKILL_SPECS,
        owner: RuntimeRouteOwner::ControlService,
        rationale: "skill-spec reads are served by control-owned registry projections",
    },
    RuntimeRouteOwnership {
        method: "POST",
        path: ROUTE_RUNTIME_SKILLS_SKILL_SPECS,
        owner: RuntimeRouteOwner::ControlService,
        rationale: "skill-spec writes remain in control-owned registry lane",
    },
    RuntimeRouteOwnership {
        method: "POST",
        path: ROUTE_RUNTIME_SKILLS_SKILL_SPEC_PUBLISH,
        owner: RuntimeRouteOwner::ControlService,
        rationale: "skill release publication is handled by control-owned registry lane",
    },
    RuntimeRouteOwnership {
        method: "GET",
        path: ROUTE_RUNTIME_SKILLS_RELEASE,
        owner: RuntimeRouteOwner::ControlService,
        rationale: "published skill bundle readback is served from control-owned registry lane",
    },
    RuntimeRouteOwnership {
        method: "GET",
        path: ROUTE_RUNTIME_THREADS,
        owner: RuntimeRouteOwner::ControlService,
        rationale: "thread projection read API is control-owned for current codex projection store",
    },
    RuntimeRouteOwnership {
        method: "GET",
        path: ROUTE_RUNTIME_THREAD_MESSAGES,
        owner: RuntimeRouteOwner::ControlService,
        rationale: "thread message projection read API is control-owned for codex projection store",
    },
    RuntimeRouteOwnership {
        method: "POST",
        path: ROUTE_RUNTIME_THREAD_MESSAGES,
        owner: RuntimeRouteOwner::ControlService,
        rationale: "thread command ingestion for codex worker lane is control-owned",
    },
    RuntimeRouteOwnership {
        method: "GET",
        path: ROUTE_RUNTIME_CODEX_WORKERS,
        owner: RuntimeRouteOwner::ControlService,
        rationale: "desktop codex worker session directory is control-owned in this lane",
    },
    RuntimeRouteOwnership {
        method: "POST",
        path: ROUTE_RUNTIME_CODEX_WORKERS,
        owner: RuntimeRouteOwner::ControlService,
        rationale: "desktop codex worker session creation is control-owned in this lane",
    },
    RuntimeRouteOwnership {
        method: "GET",
        path: ROUTE_RUNTIME_CODEX_WORKER_BY_ID,
        owner: RuntimeRouteOwner::ControlService,
        rationale: "desktop codex worker snapshot read is control-owned in this lane",
    },
    RuntimeRouteOwnership {
        method: "GET",
        path: ROUTE_RUNTIME_CODEX_WORKER_STREAM,
        owner: RuntimeRouteOwner::ControlService,
        rationale: "codex worker stream bootstrap and replay cursor lane is control-owned",
    },
    RuntimeRouteOwnership {
        method: "POST",
        path: ROUTE_RUNTIME_CODEX_WORKER_EVENTS,
        owner: RuntimeRouteOwner::ControlService,
        rationale: "desktop-originated codex worker events are ingested in control lane",
    },
    RuntimeRouteOwnership {
        method: "POST",
        path: ROUTE_RUNTIME_CODEX_WORKER_STOP,
        owner: RuntimeRouteOwner::ControlService,
        rationale: "codex worker terminal state transitions are control-owned in this lane",
    },
    RuntimeRouteOwnership {
        method: "POST",
        path: ROUTE_RUNTIME_CODEX_WORKER_REQUESTS,
        owner: RuntimeRouteOwner::ControlService,
        rationale: "allowlisted codex control requests are mediated in control lane",
    },
    RuntimeRouteOwnership {
        method: "GET",
        path: ROUTE_RUNTIME_WORKERS,
        owner: RuntimeRouteOwner::RuntimeService,
        rationale: "runtime worker authority list is proxied to runtime internal authority APIs",
    },
    RuntimeRouteOwnership {
        method: "POST",
        path: ROUTE_RUNTIME_WORKERS,
        owner: RuntimeRouteOwner::RuntimeService,
        rationale: "runtime worker authority registration is proxied to runtime internal APIs",
    },
    RuntimeRouteOwnership {
        method: "GET",
        path: ROUTE_RUNTIME_WORKER_BY_ID,
        owner: RuntimeRouteOwner::RuntimeService,
        rationale: "runtime worker authority read is proxied to runtime internal APIs",
    },
    RuntimeRouteOwnership {
        method: "POST",
        path: ROUTE_RUNTIME_WORKER_HEARTBEAT,
        owner: RuntimeRouteOwner::RuntimeService,
        rationale: "runtime worker heartbeat authority update is proxied to runtime internal APIs",
    },
    RuntimeRouteOwnership {
        method: "POST",
        path: ROUTE_RUNTIME_WORKER_STATUS,
        owner: RuntimeRouteOwner::RuntimeService,
        rationale: "runtime worker status authority update is proxied to runtime internal APIs",
    },
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
}
