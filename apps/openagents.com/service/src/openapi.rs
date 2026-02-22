use serde_json::{Map, Value, json};

pub const ROUTE_OPENAPI_JSON: &str = "/openapi.json";
pub const ROUTE_AUTH_EMAIL: &str = "/api/auth/email";
pub const ROUTE_AUTH_REGISTER: &str = "/api/auth/register";
pub const ROUTE_AUTH_VERIFY: &str = "/api/auth/verify";
pub const ROUTE_AUTH_REFRESH: &str = "/api/auth/refresh";
pub const ROUTE_AUTH_SESSION: &str = "/api/auth/session";
pub const ROUTE_AUTH_SESSIONS: &str = "/api/auth/sessions";
pub const ROUTE_AUTH_SESSIONS_REVOKE: &str = "/api/auth/sessions/revoke";
pub const ROUTE_AUTH_LOGOUT: &str = "/api/auth/logout";
pub const ROUTE_ME: &str = "/api/me";
pub const ROUTE_AUTOPILOTS: &str = "/api/autopilots";
pub const ROUTE_AUTOPILOTS_BY_ID: &str = "/api/autopilots/:autopilot";
pub const ROUTE_AUTOPILOTS_THREADS: &str = "/api/autopilots/:autopilot/threads";
pub const ROUTE_AUTOPILOTS_STREAM: &str = "/api/autopilots/:autopilot/stream";
pub const ROUTE_TOKENS: &str = "/api/tokens";
pub const ROUTE_TOKENS_CURRENT: &str = "/api/tokens/current";
pub const ROUTE_TOKENS_BY_ID: &str = "/api/tokens/:token_id";
pub const ROUTE_KHALA_TOKEN: &str = "/api/khala/token";
pub const ROUTE_ORGS_MEMBERSHIPS: &str = "/api/orgs/memberships";
pub const ROUTE_ORGS_ACTIVE: &str = "/api/orgs/active";
pub const ROUTE_POLICY_AUTHORIZE: &str = "/api/policy/authorize";
pub const ROUTE_SETTINGS_PROFILE: &str = "/api/settings/profile";
pub const ROUTE_SYNC_TOKEN: &str = "/api/sync/token";
pub const ROUTE_RUNTIME_THREADS: &str = "/api/runtime/threads";
pub const ROUTE_RUNTIME_THREAD_MESSAGES: &str = "/api/runtime/threads/:thread_id/messages";
pub const ROUTE_RUNTIME_CODEX_WORKER_REQUESTS: &str =
    "/api/runtime/codex/workers/:worker_id/requests";
pub const ROUTE_V1_AUTH_SESSION: &str = "/api/v1/auth/session";
pub const ROUTE_V1_AUTH_SESSIONS: &str = "/api/v1/auth/sessions";
pub const ROUTE_V1_AUTH_SESSIONS_REVOKE: &str = "/api/v1/auth/sessions/revoke";
pub const ROUTE_V1_CONTROL_STATUS: &str = "/api/v1/control/status";
pub const ROUTE_V1_CONTROL_ROUTE_SPLIT_STATUS: &str = "/api/v1/control/route-split/status";
pub const ROUTE_V1_CONTROL_ROUTE_SPLIT_OVERRIDE: &str = "/api/v1/control/route-split/override";
pub const ROUTE_V1_CONTROL_ROUTE_SPLIT_EVALUATE: &str = "/api/v1/control/route-split/evaluate";
pub const ROUTE_V1_SYNC_TOKEN: &str = "/api/v1/sync/token";

#[derive(Clone, Copy)]
struct OpenApiContract {
    method: &'static str,
    route_path: &'static str,
    operation_id: &'static str,
    summary: &'static str,
    tag: &'static str,
    secured: bool,
    deprecated: bool,
    success_status: &'static str,
    request_example: Option<&'static str>,
    response_example: Option<&'static str>,
}

const OPENAPI_CONTRACTS: &[OpenApiContract] = &[
    OpenApiContract {
        method: "post",
        route_path: ROUTE_AUTH_EMAIL,
        operation_id: "authEmail",
        summary: "Request a magic-code login challenge.",
        tag: "auth",
        secured: false,
        deprecated: false,
        success_status: "200",
        request_example: Some("auth_email"),
        response_example: Some("auth_email"),
    },
    OpenApiContract {
        method: "post",
        route_path: ROUTE_AUTH_REGISTER,
        operation_id: "authRegister",
        summary: "Bootstrap an API user + bearer token (local/testing only).",
        tag: "auth",
        secured: false,
        deprecated: false,
        success_status: "200",
        request_example: Some("auth_register"),
        response_example: Some("auth_register"),
    },
    OpenApiContract {
        method: "post",
        route_path: ROUTE_AUTH_VERIFY,
        operation_id: "authVerify",
        summary: "Verify a login challenge and issue session tokens.",
        tag: "auth",
        secured: false,
        deprecated: false,
        success_status: "200",
        request_example: Some("auth_verify"),
        response_example: Some("auth_verify"),
    },
    OpenApiContract {
        method: "post",
        route_path: ROUTE_AUTH_REFRESH,
        operation_id: "authRefresh",
        summary: "Rotate an authenticated refresh token.",
        tag: "auth",
        secured: false,
        deprecated: false,
        success_status: "200",
        request_example: Some("auth_refresh"),
        response_example: Some("auth_refresh"),
    },
    OpenApiContract {
        method: "get",
        route_path: ROUTE_AUTH_SESSION,
        operation_id: "authSession",
        summary: "Read the current session bundle.",
        tag: "auth",
        secured: true,
        deprecated: false,
        success_status: "200",
        request_example: None,
        response_example: Some("auth_session"),
    },
    OpenApiContract {
        method: "get",
        route_path: ROUTE_AUTH_SESSIONS,
        operation_id: "authSessionList",
        summary: "List sessions for the authenticated user.",
        tag: "auth",
        secured: true,
        deprecated: false,
        success_status: "200",
        request_example: None,
        response_example: Some("auth_sessions"),
    },
    OpenApiContract {
        method: "post",
        route_path: ROUTE_AUTH_SESSIONS_REVOKE,
        operation_id: "authSessionRevoke",
        summary: "Revoke one, device, or all sessions.",
        tag: "auth",
        secured: true,
        deprecated: false,
        success_status: "200",
        request_example: Some("auth_sessions_revoke"),
        response_example: Some("auth_sessions_revoke"),
    },
    OpenApiContract {
        method: "post",
        route_path: ROUTE_AUTH_LOGOUT,
        operation_id: "authLogout",
        summary: "Logout the current session.",
        tag: "auth",
        secured: true,
        deprecated: false,
        success_status: "200",
        request_example: None,
        response_example: Some("auth_logout"),
    },
    OpenApiContract {
        method: "get",
        route_path: ROUTE_ME,
        operation_id: "me",
        summary: "Read authenticated user profile and chat thread summaries.",
        tag: "identity",
        secured: true,
        deprecated: false,
        success_status: "200",
        request_example: None,
        response_example: Some("me"),
    },
    OpenApiContract {
        method: "get",
        route_path: ROUTE_AUTOPILOTS,
        operation_id: "autopilotsList",
        summary: "List autopilots owned by the authenticated user.",
        tag: "autopilot",
        secured: true,
        deprecated: false,
        success_status: "200",
        request_example: None,
        response_example: Some("autopilot"),
    },
    OpenApiContract {
        method: "post",
        route_path: ROUTE_AUTOPILOTS,
        operation_id: "autopilotsCreate",
        summary: "Create an autopilot for the authenticated user.",
        tag: "autopilot",
        secured: true,
        deprecated: false,
        success_status: "201",
        request_example: Some("autopilot_create"),
        response_example: Some("autopilot"),
    },
    OpenApiContract {
        method: "get",
        route_path: ROUTE_AUTOPILOTS_BY_ID,
        operation_id: "autopilotsShow",
        summary: "Read one owned autopilot by id or handle.",
        tag: "autopilot",
        secured: true,
        deprecated: false,
        success_status: "200",
        request_example: None,
        response_example: Some("autopilot"),
    },
    OpenApiContract {
        method: "patch",
        route_path: ROUTE_AUTOPILOTS_BY_ID,
        operation_id: "autopilotsUpdate",
        summary: "Update one owned autopilot.",
        tag: "autopilot",
        secured: true,
        deprecated: false,
        success_status: "200",
        request_example: Some("autopilot_update"),
        response_example: Some("autopilot"),
    },
    OpenApiContract {
        method: "get",
        route_path: ROUTE_AUTOPILOTS_THREADS,
        operation_id: "autopilotThreadsList",
        summary: "List threads for one owned autopilot.",
        tag: "autopilot",
        secured: true,
        deprecated: false,
        success_status: "200",
        request_example: None,
        response_example: Some("autopilot_threads"),
    },
    OpenApiContract {
        method: "post",
        route_path: ROUTE_AUTOPILOTS_THREADS,
        operation_id: "autopilotThreadsCreate",
        summary: "Create a thread for one owned autopilot.",
        tag: "autopilot",
        secured: true,
        deprecated: false,
        success_status: "201",
        request_example: Some("autopilot_thread_create"),
        response_example: Some("autopilot_thread"),
    },
    OpenApiContract {
        method: "post",
        route_path: ROUTE_AUTOPILOTS_STREAM,
        operation_id: "autopilotStreamBootstrap",
        summary: "Bootstrap autopilot turn execution and consume live updates via Khala WS.",
        tag: "autopilot",
        secured: true,
        deprecated: false,
        success_status: "200",
        request_example: Some("autopilot_stream"),
        response_example: Some("autopilot_stream_bootstrap"),
    },
    OpenApiContract {
        method: "get",
        route_path: ROUTE_TOKENS,
        operation_id: "tokensList",
        summary: "List personal access tokens for the authenticated user.",
        tag: "auth",
        secured: true,
        deprecated: false,
        success_status: "200",
        request_example: None,
        response_example: Some("tokens_list"),
    },
    OpenApiContract {
        method: "post",
        route_path: ROUTE_TOKENS,
        operation_id: "tokensCreate",
        summary: "Create a personal access token and return the plaintext token once.",
        tag: "auth",
        secured: true,
        deprecated: false,
        success_status: "201",
        request_example: Some("tokens_create"),
        response_example: Some("tokens_create"),
    },
    OpenApiContract {
        method: "delete",
        route_path: ROUTE_TOKENS_CURRENT,
        operation_id: "tokensDestroyCurrent",
        summary: "Revoke the current bearer personal access token.",
        tag: "auth",
        secured: true,
        deprecated: false,
        success_status: "200",
        request_example: None,
        response_example: Some("tokens_delete_current"),
    },
    OpenApiContract {
        method: "delete",
        route_path: ROUTE_TOKENS_BY_ID,
        operation_id: "tokensDestroy",
        summary: "Revoke a specific personal access token by id.",
        tag: "auth",
        secured: true,
        deprecated: false,
        success_status: "200",
        request_example: None,
        response_example: Some("tokens_delete"),
    },
    OpenApiContract {
        method: "delete",
        route_path: ROUTE_TOKENS,
        operation_id: "tokensDestroyAll",
        summary: "Revoke all personal access tokens for the authenticated user.",
        tag: "auth",
        secured: true,
        deprecated: false,
        success_status: "200",
        request_example: None,
        response_example: Some("tokens_delete_all"),
    },
    OpenApiContract {
        method: "post",
        route_path: ROUTE_KHALA_TOKEN,
        operation_id: "khalaToken",
        summary: "Issue a short-lived Khala identity token.",
        tag: "auth",
        secured: true,
        deprecated: false,
        success_status: "200",
        request_example: Some("khala_token"),
        response_example: Some("khala_token"),
    },
    OpenApiContract {
        method: "get",
        route_path: ROUTE_SETTINGS_PROFILE,
        operation_id: "settingsProfileShow",
        summary: "Read the authenticated user profile.",
        tag: "profile",
        secured: true,
        deprecated: false,
        success_status: "200",
        request_example: None,
        response_example: Some("settings_profile"),
    },
    OpenApiContract {
        method: "patch",
        route_path: ROUTE_SETTINGS_PROFILE,
        operation_id: "settingsProfileUpdate",
        summary: "Update profile fields for the authenticated user.",
        tag: "profile",
        secured: true,
        deprecated: false,
        success_status: "200",
        request_example: Some("settings_profile_update"),
        response_example: Some("settings_profile"),
    },
    OpenApiContract {
        method: "delete",
        route_path: ROUTE_SETTINGS_PROFILE,
        operation_id: "settingsProfileDelete",
        summary: "Delete the authenticated user account.",
        tag: "profile",
        secured: true,
        deprecated: false,
        success_status: "200",
        request_example: Some("settings_profile_delete"),
        response_example: Some("profile_deleted"),
    },
    OpenApiContract {
        method: "get",
        route_path: ROUTE_ORGS_MEMBERSHIPS,
        operation_id: "orgMemberships",
        summary: "List organization memberships.",
        tag: "identity",
        secured: true,
        deprecated: false,
        success_status: "200",
        request_example: None,
        response_example: Some("org_memberships"),
    },
    OpenApiContract {
        method: "post",
        route_path: ROUTE_ORGS_ACTIVE,
        operation_id: "setActiveOrg",
        summary: "Set active organization for the current session.",
        tag: "identity",
        secured: true,
        deprecated: false,
        success_status: "200",
        request_example: Some("orgs_active"),
        response_example: Some("auth_session"),
    },
    OpenApiContract {
        method: "post",
        route_path: ROUTE_POLICY_AUTHORIZE,
        operation_id: "policyAuthorize",
        summary: "Evaluate requested scopes/topics for an authenticated principal.",
        tag: "policy",
        secured: true,
        deprecated: false,
        success_status: "200",
        request_example: Some("policy_authorize"),
        response_example: Some("policy_authorize"),
    },
    OpenApiContract {
        method: "post",
        route_path: ROUTE_SYNC_TOKEN,
        operation_id: "syncToken",
        summary: "Issue a Khala WebSocket sync token.",
        tag: "sync",
        secured: true,
        deprecated: false,
        success_status: "200",
        request_example: Some("sync_token"),
        response_example: Some("sync_token"),
    },
    OpenApiContract {
        method: "get",
        route_path: ROUTE_RUNTIME_THREADS,
        operation_id: "runtimeThreadsList",
        summary: "List Codex thread projections for the current user.",
        tag: "codex",
        secured: true,
        deprecated: false,
        success_status: "200",
        request_example: None,
        response_example: Some("runtime_threads"),
    },
    OpenApiContract {
        method: "get",
        route_path: ROUTE_RUNTIME_THREAD_MESSAGES,
        operation_id: "runtimeThreadMessagesList",
        summary: "List projected messages for a Codex thread.",
        tag: "codex",
        secured: true,
        deprecated: false,
        success_status: "200",
        request_example: None,
        response_example: Some("runtime_thread_messages"),
    },
    OpenApiContract {
        method: "post",
        route_path: ROUTE_RUNTIME_THREAD_MESSAGES,
        operation_id: "runtimeThreadMessage",
        summary: "Submit a Codex thread message command.",
        tag: "codex",
        secured: true,
        deprecated: false,
        success_status: "200",
        request_example: Some("thread_message"),
        response_example: Some("thread_message"),
    },
    OpenApiContract {
        method: "post",
        route_path: ROUTE_RUNTIME_CODEX_WORKER_REQUESTS,
        operation_id: "runtimeCodexWorkerRequest",
        summary: "Dispatch an allowlisted Codex worker control request.",
        tag: "codex",
        secured: true,
        deprecated: false,
        success_status: "200",
        request_example: Some("runtime_codex_worker_request"),
        response_example: Some("runtime_codex_worker_request"),
    },
    OpenApiContract {
        method: "get",
        route_path: ROUTE_V1_AUTH_SESSION,
        operation_id: "v1AuthSession",
        summary: "Compatibility alias for auth session.",
        tag: "compat",
        secured: true,
        deprecated: true,
        success_status: "200",
        request_example: None,
        response_example: Some("auth_session"),
    },
    OpenApiContract {
        method: "get",
        route_path: ROUTE_V1_AUTH_SESSIONS,
        operation_id: "v1AuthSessions",
        summary: "Compatibility alias for auth session list.",
        tag: "compat",
        secured: true,
        deprecated: true,
        success_status: "200",
        request_example: None,
        response_example: Some("auth_sessions"),
    },
    OpenApiContract {
        method: "post",
        route_path: ROUTE_V1_AUTH_SESSIONS_REVOKE,
        operation_id: "v1AuthSessionsRevoke",
        summary: "Compatibility alias for session revocation.",
        tag: "compat",
        secured: true,
        deprecated: true,
        success_status: "200",
        request_example: Some("auth_sessions_revoke"),
        response_example: Some("auth_sessions_revoke"),
    },
    OpenApiContract {
        method: "get",
        route_path: ROUTE_V1_CONTROL_STATUS,
        operation_id: "controlStatus",
        summary: "Read runtime control status.",
        tag: "control",
        secured: true,
        deprecated: false,
        success_status: "200",
        request_example: None,
        response_example: Some("control_status"),
    },
    OpenApiContract {
        method: "get",
        route_path: ROUTE_V1_CONTROL_ROUTE_SPLIT_STATUS,
        operation_id: "controlRouteSplitStatus",
        summary: "Read route-split control status.",
        tag: "control",
        secured: true,
        deprecated: false,
        success_status: "200",
        request_example: None,
        response_example: Some("route_split_status"),
    },
    OpenApiContract {
        method: "post",
        route_path: ROUTE_V1_CONTROL_ROUTE_SPLIT_OVERRIDE,
        operation_id: "controlRouteSplitOverride",
        summary: "Set route-split override target.",
        tag: "control",
        secured: true,
        deprecated: false,
        success_status: "200",
        request_example: Some("route_split_override"),
        response_example: Some("route_split_status"),
    },
    OpenApiContract {
        method: "post",
        route_path: ROUTE_V1_CONTROL_ROUTE_SPLIT_EVALUATE,
        operation_id: "controlRouteSplitEvaluate",
        summary: "Evaluate route-split decision for a path.",
        tag: "control",
        secured: true,
        deprecated: false,
        success_status: "200",
        request_example: Some("route_split_evaluate"),
        response_example: Some("route_split_evaluate"),
    },
    OpenApiContract {
        method: "post",
        route_path: ROUTE_V1_SYNC_TOKEN,
        operation_id: "v1SyncToken",
        summary: "Compatibility alias for sync token issuance.",
        tag: "compat",
        secured: true,
        deprecated: true,
        success_status: "200",
        request_example: Some("sync_token"),
        response_example: Some("sync_token"),
    },
    OpenApiContract {
        method: "get",
        route_path: ROUTE_OPENAPI_JSON,
        operation_id: "openapiDocument",
        summary: "Read generated OpenAPI document.",
        tag: "utility",
        secured: false,
        deprecated: false,
        success_status: "200",
        request_example: None,
        response_example: None,
    },
];

pub fn openapi_document() -> Value {
    let mut paths = Map::new();
    for contract in OPENAPI_CONTRACTS {
        add_operation(&mut paths, contract);
    }

    json!({
        "openapi": "3.0.2",
        "info": {
            "title": "OpenAgents Control API",
            "version": env!("CARGO_PKG_VERSION"),
            "description": "Rust-owned control API for auth/session, policy, sync, control-plane route split, and Codex command bootstrap."
        },
        "servers": [
            { "url": "https://openagents.com" }
        ],
        "paths": Value::Object(paths),
        "components": {
            "securitySchemes": {
                "bearerAuth": {
                    "type": "http",
                    "scheme": "bearer",
                    "bearerFormat": "token"
                },
                "sessionCookie": {
                    "type": "apiKey",
                    "in": "cookie",
                    "name": "laravel_session"
                },
                "csrfHeader": {
                    "type": "apiKey",
                    "in": "header",
                    "name": "x-xsrf-token"
                }
            },
            "schemas": {
                "ApiDataEnvelope": {
                    "type": "object",
                    "properties": {
                        "data": { "type": "object", "additionalProperties": true }
                    },
                    "required": ["data"]
                },
                "ApiErrorResponse": {
                    "type": "object",
                    "properties": {
                        "message": { "type": "string" },
                        "error": {
                            "type": "object",
                            "properties": {
                                "code": { "type": "string" },
                                "message": { "type": "string" }
                            },
                            "required": ["code", "message"]
                        },
                        "errors": {
                            "type": "object",
                            "additionalProperties": {
                                "type": "array",
                                "items": { "type": "string" }
                            }
                        }
                    },
                    "required": ["message", "error"]
                }
            },
            "responses": {
                "ErrorEnvelope": {
                    "description": "Error response envelope",
                    "content": {
                        "application/json": {
                            "schema": { "$ref": "#/components/schemas/ApiErrorResponse" },
                            "example": {
                                "message": "Unauthenticated.",
                                "error": {
                                    "code": "unauthorized",
                                    "message": "Unauthenticated."
                                }
                            }
                        }
                    }
                }
            }
        }
    })
}

fn add_operation(paths: &mut Map<String, Value>, contract: &OpenApiContract) {
    let path = to_openapi_path(contract.route_path);
    let method = contract.method.to_ascii_lowercase();

    let mut operation = json!({
        "operationId": contract.operation_id,
        "summary": contract.summary,
        "tags": [contract.tag],
        "responses": {
            contract.success_status: {
                "description": "Success",
                "content": {
                    "application/json": {
                        "schema": { "$ref": "#/components/schemas/ApiDataEnvelope" }
                    }
                }
            },
            "default": { "$ref": "#/components/responses/ErrorEnvelope" }
        },
        "x-rustRoute": contract.route_path,
    });

    if contract.secured {
        operation["security"] = protected_security();
    }

    if contract.deprecated {
        operation["deprecated"] = Value::Bool(true);
        operation["x-deprecated-reason"] = Value::String(
            "Compatibility alias retained during Laravel-to-Rust migration.".to_string(),
        );
    }

    if let Some(example_key) = contract.request_example {
        let mut request_body = json!({
            "required": true,
            "content": {
                "application/json": {
                    "schema": { "type": "object", "additionalProperties": true }
                }
            }
        });
        if let Some(example) = request_example(example_key) {
            request_body["content"]["application/json"]["example"] = example;
        }
        operation["requestBody"] = request_body;
    }

    if let Some(example_key) = contract.response_example {
        if let Some(example) = response_example(example_key) {
            operation["responses"][contract.success_status]["content"]["application/json"]["example"] =
                example;
        }
    }

    let parameters = path_parameters(contract.route_path);
    if !parameters.is_empty() {
        operation["parameters"] = Value::Array(parameters);
    }

    let path_item = paths
        .entry(path)
        .or_insert_with(|| Value::Object(Map::new()));
    if let Some(item) = path_item.as_object_mut() {
        item.insert(method, operation);
    }
}

fn protected_security() -> Value {
    json!([
        {"bearerAuth": []},
        {"sessionCookie": [], "csrfHeader": []}
    ])
}

fn to_openapi_path(route_path: &str) -> String {
    route_path
        .split('/')
        .map(|segment| {
            if let Some(parameter) = segment.strip_prefix(':') {
                format!("{{{parameter}}}")
            } else {
                segment.to_string()
            }
        })
        .collect::<Vec<String>>()
        .join("/")
}

fn path_parameters(route_path: &str) -> Vec<Value> {
    route_path
        .split('/')
        .filter_map(|segment| segment.strip_prefix(':'))
        .map(|parameter| {
            json!({
                "name": parameter,
                "in": "path",
                "required": true,
                "schema": {"type": "string"}
            })
        })
        .collect()
}

fn request_example(key: &str) -> Option<Value> {
    match key {
        "auth_email" => Some(json!({ "email": "user@openagents.com" })),
        "auth_register" => Some(json!({
            "email": "staging-user@staging.openagents.com",
            "name": "Staging User",
            "tokenName": "staging-e2e",
            "tokenAbilities": ["*"],
            "createAutopilot": true,
            "autopilotDisplayName": "Creator Agent"
        })),
        "auth_verify" => Some(json!({ "code": "123456", "device_id": "ios:device" })),
        "auth_refresh" => Some(json!({
            "refresh_token": "oa_rt_123",
            "rotate_refresh_token": true,
            "device_id": "ios:device"
        })),
        "auth_sessions_revoke" => Some(json!({
            "revoke_all_sessions": true,
            "include_current": false,
            "reason": "security_policy"
        })),
        "tokens_create" => Some(json!({
            "name": "api-cli",
            "abilities": ["chat:read", "chat:write"],
            "expires_at": "2026-03-01T00:00:00Z"
        })),
        "khala_token" => Some(json!({
            "scope": ["codex:read", "codex:write"],
            "workspace_id": "workspace_42",
            "role": "admin"
        })),
        "settings_profile_update" => Some(json!({
            "name": "Updated Name"
        })),
        "settings_profile_delete" => Some(json!({
            "email": "user@openagents.com"
        })),
        "orgs_active" => Some(json!({ "org_id": "org:openagents" })),
        "policy_authorize" => Some(json!({
            "org_id": "org:openagents",
            "required_scopes": ["runtime.read"],
            "requested_topics": ["org:openagents:worker_events"]
        })),
        "sync_token" => Some(json!({
            "scopes": ["runtime.codex_worker_events"],
            "topics": ["org:openagents:worker_events"],
            "ttl_seconds": 300,
            "device_id": "ios:device"
        })),
        "thread_message" => Some(json!({ "text": "Summarize this thread." })),
        "runtime_codex_worker_request" => Some(json!({
            "request": {
                "request_id": "req_turn_1",
                "method": "turn/start",
                "params": {
                    "thread_id": "thread_123",
                    "text": "Continue from last checkpoint."
                },
                "request_version": "v1",
                "source": "openagents-web-shell"
            }
        })),
        "route_split_override" => Some(json!({
            "target": "rollback",
            "domain": "billing_l402"
        })),
        "route_split_evaluate" => Some(json!({
            "path": "/chat/thread-1",
            "cohort_key": "user:123"
        })),
        "autopilot_create" => Some(json!({
            "handle": "ep212-bot",
            "displayName": "EP212 Bot",
            "status": "active",
            "visibility": "private"
        })),
        "autopilot_update" => Some(json!({
            "displayName": "EP212 Bot Updated",
            "profile": {
                "ownerDisplayName": "Chris",
                "personaSummary": "Pragmatic and concise",
                "autopilotVoice": "calm and direct"
            },
            "policy": {
                "toolAllowlist": ["openagents_api"],
                "l402RequireApproval": true
            }
        })),
        "autopilot_thread_create" => Some(json!({
            "title": "Autopilot test thread"
        })),
        "autopilot_stream" => Some(json!({
            "conversationId": "thread_123",
            "messages": [
                {"id": "m1", "role": "user", "content": "hello from autopilot stream alias"}
            ]
        })),
        _ => None,
    }
}

fn response_example(key: &str) -> Option<Value> {
    match key {
        "auth_email" => Some(json!({
            "data": {
                "status": "ok",
                "challengeId": "challenge_123",
                "email": "user@openagents.com"
            }
        })),
        "auth_register" => Some(json!({
            "data": {
                "created": true,
                "tokenType": "Bearer",
                "token": "oa_pat_123",
                "tokenName": "staging-e2e",
                "tokenAbilities": ["*"],
                "user": {
                    "id": "user_123",
                    "name": "Staging User",
                    "email": "staging-user@staging.openagents.com",
                    "handle": "staging-user"
                },
                "autopilot": {
                    "id": "ap_123",
                    "handle": "creator-agent",
                    "displayName": "Creator Agent",
                    "status": "active",
                    "visibility": "private"
                }
            }
        })),
        "auth_verify" => Some(json!({
            "data": {
                "status": "authenticated",
                "token": "oa_at_123",
                "refreshToken": "oa_rt_123",
                "tokenType": "Bearer"
            }
        })),
        "auth_refresh" => Some(json!({
            "data": {
                "token": "oa_at_rotated",
                "refreshToken": "oa_rt_rotated",
                "tokenType": "Bearer"
            }
        })),
        "auth_session" => Some(json!({
            "data": {
                "session": {
                    "id": "sess_123",
                    "status": "active"
                },
                "user": {
                    "id": "usr_123",
                    "email": "user@openagents.com"
                },
                "memberships": []
            }
        })),
        "auth_sessions" => Some(json!({
            "data": {
                "sessions": [
                    {"sessionId": "sess_123", "status": "active"}
                ]
            }
        })),
        "auth_sessions_revoke" => Some(json!({
            "data": {
                "revokedSessionIds": ["sess_123"],
                "reason": "security_policy"
            }
        })),
        "auth_logout" => Some(json!({
            "data": {
                "status": "logged_out"
            }
        })),
        "me" => Some(json!({
            "data": {
                "user": {
                    "id": "usr_123",
                    "name": "OpenAgents User",
                    "email": "user@openagents.com",
                    "avatar": "",
                    "createdAt": null,
                    "updatedAt": null
                },
                "chatThreads": [
                    {
                        "id": "thread-1",
                        "title": "Thread thread-1",
                        "updatedAt": "2026-02-22T00:00:00Z"
                    }
                ]
            }
        })),
        "autopilot" => Some(json!({
            "data": {
                "id": "ap_123",
                "handle": "ep212-bot",
                "displayName": "EP212 Bot",
                "status": "active",
                "visibility": "private",
                "ownerUserId": "usr_123",
                "avatar": null,
                "tagline": null,
                "configVersion": 2,
                "profile": {
                    "ownerDisplayName": "Chris",
                    "personaSummary": "Pragmatic and concise",
                    "autopilotVoice": "calm and direct",
                    "principles": [],
                    "preferences": [],
                    "onboardingAnswers": [],
                    "schemaVersion": 1
                },
                "policy": {
                    "modelProvider": null,
                    "model": null,
                    "toolAllowlist": ["openagents_api"],
                    "toolDenylist": [],
                    "l402RequireApproval": true,
                    "l402MaxSpendMsatsPerCall": null,
                    "l402MaxSpendMsatsPerDay": null,
                    "l402AllowedHosts": [],
                    "dataPolicy": []
                },
                "createdAt": "2026-02-22T00:00:00Z",
                "updatedAt": "2026-02-22T00:00:00Z"
            }
        })),
        "autopilot_thread" => Some(json!({
            "data": {
                "id": "thread_123",
                "autopilotId": "ap_123",
                "title": "Autopilot test thread",
                "createdAt": "2026-02-22T00:00:00Z",
                "updatedAt": "2026-02-22T00:00:00Z"
            }
        })),
        "autopilot_threads" => Some(json!({
            "data": [
                {
                    "id": "thread_123",
                    "autopilotId": "ap_123",
                    "title": "Autopilot test thread",
                    "createdAt": "2026-02-22T00:00:00Z",
                    "updatedAt": "2026-02-22T00:00:00Z"
                }
            ]
        })),
        "autopilot_stream_bootstrap" => Some(json!({
            "data": {
                "accepted": true,
                "autopilotId": "ap_123",
                "autopilotConfigVersion": 2,
                "threadId": "thread_123",
                "conversationId": "thread_123",
                "streamProtocol": "disabled",
                "delivery": {
                    "transport": "khala_ws",
                    "topic": "org:openagents:worker_events",
                    "scope": "runtime.codex_worker_events",
                    "syncTokenRoute": "/api/sync/token"
                },
                "control": {
                    "method": "turn/start",
                    "workerId": "desktopw:shared",
                    "requestId": "autopilot_stream_123"
                },
                "response": {
                    "thread_id": "thread_123",
                    "turn": {"id": "turn_456"},
                    "message": {
                        "id": "msg_1",
                        "thread_id": "thread_123",
                        "role": "user",
                        "text": "hello from autopilot stream alias"
                    }
                }
            }
        })),
        "tokens_list" => Some(json!({
            "data": [
                {
                    "id": "pat_123",
                    "name": "api-cli",
                    "abilities": ["chat:read", "chat:write"],
                    "lastUsedAt": "2026-02-22T00:00:00Z",
                    "expiresAt": "2026-03-01T00:00:00Z",
                    "createdAt": "2026-02-22T00:00:00Z",
                    "isCurrent": true
                }
            ]
        })),
        "tokens_create" => Some(json!({
            "data": {
                "token": "oa_pat_123",
                "tokenableId": "user_123",
                "name": "api-cli",
                "abilities": ["chat:read", "chat:write"],
                "expiresAt": "2026-03-01T00:00:00Z"
            }
        })),
        "tokens_delete_current" => Some(json!({
            "data": {
                "deleted": true
            }
        })),
        "tokens_delete" => Some(json!({
            "data": {
                "deleted": true
            }
        })),
        "tokens_delete_all" => Some(json!({
            "data": {
                "deletedCount": 2
            }
        })),
        "khala_token" => Some(json!({
            "data": {
                "token": "eyJ...",
                "token_type": "Bearer",
                "expires_in": 300,
                "issued_at": "2026-02-22T00:00:00Z",
                "expires_at": "2026-02-22T00:05:00Z",
                "issuer": "https://openagents.test",
                "audience": "openagents-khala-test",
                "subject": "user:user_123",
                "claims_version": "oa_khala_claims_v1",
                "scope": ["codex:read", "codex:write"],
                "workspace_id": "workspace_42",
                "role": "admin",
                "kid": "khala-auth-test-v1"
            }
        })),
        "settings_profile" => Some(json!({
            "data": {
                "id": "usr_123",
                "name": "OpenAgents User",
                "email": "user@openagents.com",
                "avatar": "",
                "createdAt": null,
                "updatedAt": "2026-02-22T00:00:00Z"
            }
        })),
        "profile_deleted" => Some(json!({
            "data": {
                "deleted": true
            }
        })),
        "org_memberships" => Some(json!({
            "data": {
                "memberships": [
                    {"org_id": "org:openagents", "role": "admin"}
                ]
            }
        })),
        "policy_authorize" => Some(json!({
            "data": {
                "allowed": true,
                "resolved_org_id": "org:openagents",
                "granted_scopes": ["runtime.read"],
                "denied_reasons": []
            }
        })),
        "sync_token" => Some(json!({
            "data": {
                "token": "eyJ...",
                "token_type": "Bearer",
                "expires_at": "2026-02-22T00:00:00Z"
            }
        })),
        "thread_message" => Some(json!({
            "data": {
                "status": "accepted",
                "thread_id": "thread-1"
            }
        })),
        "runtime_codex_worker_request" => Some(json!({
            "data": {
                "worker_id": "desktopw:shared",
                "request_id": "req_turn_1",
                "ok": true,
                "method": "turn/start",
                "idempotent_replay": false,
                "response": {
                    "thread_id": "thread_123",
                    "turn": { "id": "turn_456" }
                }
            }
        })),
        "runtime_threads" => Some(json!({
            "data": {
                "threads": [
                    {
                        "thread_id": "thread-1",
                        "message_count": 3
                    }
                ]
            }
        })),
        "runtime_thread_messages" => Some(json!({
            "data": {
                "thread_id": "thread-1",
                "messages": [
                    {
                        "message_id": "msg_1",
                        "role": "user",
                        "text": "hello"
                    }
                ]
            }
        })),
        "control_status" => Some(json!({
            "data": {
                "status": "ok",
                "service": "openagents-control-service"
            }
        })),
        "route_split_status" => Some(json!({
            "data": {
                "mode": "cohort",
                "override_target": null,
                "rollback_matrix": {
                    "auth_entry": "legacy",
                    "billing_l402": "legacy",
                    "chat_pilot": "rust_shell"
                },
                "route_groups": [
                    {
                        "domain": "billing_l402",
                        "route_prefixes": ["/billing", "/l402"],
                        "rollback_target": "legacy",
                        "override_target": null
                    }
                ]
            }
        })),
        "route_split_evaluate" => Some(json!({
            "data": {
                "target": "rust_shell",
                "reason": "mode_cohort",
                "route_domain": "chat_pilot",
                "rollback_target": "rust_shell"
            }
        })),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn document_is_valid_openapi_shape() {
        let document = openapi_document();

        let openapi = document.get("openapi").and_then(Value::as_str);
        assert_eq!(openapi, Some("3.0.2"));
        assert!(document.get("paths").and_then(Value::as_object).is_some());
        assert!(
            document
                .get("components")
                .and_then(|value| value.get("schemas"))
                .and_then(Value::as_object)
                .is_some()
        );
    }

    #[test]
    fn document_deserializes_via_openapiv3_model() {
        let document = openapi_document();
        let parsed = serde_json::from_value::<openapiv3::OpenAPI>(document);
        assert!(parsed.is_ok());
    }

    #[test]
    fn includes_security_scheme_parity() {
        let document = openapi_document();
        let schemes = document
            .get("components")
            .and_then(|value| value.get("securitySchemes"))
            .and_then(Value::as_object);

        assert!(schemes.is_some());
        if let Some(schemes) = schemes {
            assert!(schemes.contains_key("bearerAuth"));
            assert!(schemes.contains_key("sessionCookie"));
            assert!(schemes.contains_key("csrfHeader"));
        }
    }

    #[test]
    fn includes_deprecated_v1_alias_visibility() {
        let document = openapi_document();
        let paths = document
            .get("paths")
            .and_then(Value::as_object)
            .cloned()
            .unwrap_or_default();

        let auth_session = paths
            .get(ROUTE_V1_AUTH_SESSION)
            .and_then(Value::as_object)
            .and_then(|path| path.get("get"))
            .and_then(Value::as_object);
        assert!(auth_session.is_some());

        let deprecated = auth_session
            .and_then(|operation| operation.get("deprecated"))
            .and_then(Value::as_bool);
        assert_eq!(deprecated, Some(true));
    }

    #[test]
    fn includes_key_endpoint_examples() {
        let document = openapi_document();

        let email_example = document
            .get("paths")
            .and_then(|paths| paths.get(ROUTE_AUTH_EMAIL))
            .and_then(|path| path.get("post"))
            .and_then(|post| post.get("requestBody"))
            .and_then(|body| body.get("content"))
            .and_then(|content| content.get("application/json"))
            .and_then(|content| content.get("example"));
        assert!(email_example.is_some());

        let sync_example = document
            .get("paths")
            .and_then(|paths| paths.get(ROUTE_SYNC_TOKEN))
            .and_then(|path| path.get("post"))
            .and_then(|post| post.get("requestBody"))
            .and_then(|body| body.get("content"))
            .and_then(|content| content.get("application/json"))
            .and_then(|content| content.get("example"));
        assert!(sync_example.is_some());

        let register_example = document
            .get("paths")
            .and_then(|paths| paths.get(ROUTE_AUTH_REGISTER))
            .and_then(|path| path.get("post"))
            .and_then(|post| post.get("requestBody"))
            .and_then(|body| body.get("content"))
            .and_then(|content| content.get("application/json"))
            .and_then(|content| content.get("example"));
        assert!(register_example.is_some());
    }
}
