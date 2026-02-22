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
pub const ROUTE_AGENT_PAYMENTS_WALLET: &str = "/api/agent-payments/wallet";
pub const ROUTE_AGENT_PAYMENTS_BALANCE: &str = "/api/agent-payments/balance";
pub const ROUTE_AGENT_PAYMENTS_INVOICE: &str = "/api/agent-payments/invoice";
pub const ROUTE_AGENT_PAYMENTS_PAY: &str = "/api/agent-payments/pay";
pub const ROUTE_AGENT_PAYMENTS_SEND_SPARK: &str = "/api/agent-payments/send-spark";
pub const ROUTE_AGENTS_ME_WALLET: &str = "/api/agents/me/wallet";
pub const ROUTE_AGENTS_ME_BALANCE: &str = "/api/agents/me/balance";
pub const ROUTE_PAYMENTS_INVOICE: &str = "/api/payments/invoice";
pub const ROUTE_PAYMENTS_PAY: &str = "/api/payments/pay";
pub const ROUTE_PAYMENTS_SEND_SPARK: &str = "/api/payments/send-spark";
pub const ROUTE_SHOUTS: &str = "/api/shouts";
pub const ROUTE_SHOUTS_ZONES: &str = "/api/shouts/zones";
pub const ROUTE_L402_WALLET: &str = "/api/l402/wallet";
pub const ROUTE_L402_TRANSACTIONS: &str = "/api/l402/transactions";
pub const ROUTE_L402_TRANSACTION_BY_ID: &str = "/api/l402/transactions/:eventId";
pub const ROUTE_L402_PAYWALLS: &str = "/api/l402/paywalls";
pub const ROUTE_L402_PAYWALL_BY_ID: &str = "/api/l402/paywalls/:paywallId";
pub const ROUTE_L402_SETTLEMENTS: &str = "/api/l402/settlements";
pub const ROUTE_L402_DEPLOYMENTS: &str = "/api/l402/deployments";
pub const ROUTE_SETTINGS_PROFILE: &str = "/api/settings/profile";
pub const ROUTE_SETTINGS_AUTOPILOT: &str = "/settings/autopilot";
pub const ROUTE_SYNC_TOKEN: &str = "/api/sync/token";
pub const ROUTE_RUNTIME_INTERNAL_SECRET_FETCH: &str =
    "/api/internal/runtime/integrations/secrets/fetch";
pub const ROUTE_LIGHTNING_OPS_CONTROL_PLANE_QUERY: &str =
    "/api/internal/lightning-ops/control-plane/query";
pub const ROUTE_LIGHTNING_OPS_CONTROL_PLANE_MUTATION: &str =
    "/api/internal/lightning-ops/control-plane/mutation";
pub const ROUTE_RUNTIME_TOOLS_EXECUTE: &str = "/api/runtime/tools/execute";
pub const ROUTE_RUNTIME_SKILLS_TOOL_SPECS: &str = "/api/runtime/skills/tool-specs";
pub const ROUTE_RUNTIME_SKILLS_SKILL_SPECS: &str = "/api/runtime/skills/skill-specs";
pub const ROUTE_RUNTIME_SKILLS_SKILL_SPEC_PUBLISH: &str =
    "/api/runtime/skills/skill-specs/:skill_id/:version/publish";
pub const ROUTE_RUNTIME_SKILLS_RELEASE: &str = "/api/runtime/skills/releases/:skill_id/:version";
pub const ROUTE_RUNTIME_CODEX_WORKERS: &str = "/api/runtime/codex/workers";
pub const ROUTE_RUNTIME_CODEX_WORKER_BY_ID: &str = "/api/runtime/codex/workers/:worker_id";
pub const ROUTE_RUNTIME_CODEX_WORKER_STREAM: &str = "/api/runtime/codex/workers/:worker_id/stream";
pub const ROUTE_RUNTIME_THREADS: &str = "/api/runtime/threads";
pub const ROUTE_RUNTIME_THREAD_MESSAGES: &str = "/api/runtime/threads/:thread_id/messages";
pub const ROUTE_RUNTIME_CODEX_WORKER_REQUESTS: &str =
    "/api/runtime/codex/workers/:worker_id/requests";
pub const ROUTE_RUNTIME_CODEX_WORKER_EVENTS: &str = "/api/runtime/codex/workers/:worker_id/events";
pub const ROUTE_RUNTIME_CODEX_WORKER_STOP: &str = "/api/runtime/codex/workers/:worker_id/stop";
pub const ROUTE_V1_AUTH_SESSION: &str = "/api/v1/auth/session";
pub const ROUTE_V1_AUTH_SESSIONS: &str = "/api/v1/auth/sessions";
pub const ROUTE_V1_AUTH_SESSIONS_REVOKE: &str = "/api/v1/auth/sessions/revoke";
pub const ROUTE_V1_CONTROL_STATUS: &str = "/api/v1/control/status";
pub const ROUTE_V1_CONTROL_ROUTE_SPLIT_STATUS: &str = "/api/v1/control/route-split/status";
pub const ROUTE_V1_CONTROL_ROUTE_SPLIT_OVERRIDE: &str = "/api/v1/control/route-split/override";
pub const ROUTE_V1_CONTROL_ROUTE_SPLIT_EVALUATE: &str = "/api/v1/control/route-split/evaluate";
pub const ROUTE_V1_CONTROL_RUNTIME_ROUTING_STATUS: &str = "/api/v1/control/runtime-routing/status";
pub const ROUTE_V1_CONTROL_RUNTIME_ROUTING_OVERRIDE: &str =
    "/api/v1/control/runtime-routing/override";
pub const ROUTE_V1_CONTROL_RUNTIME_ROUTING_EVALUATE: &str =
    "/api/v1/control/runtime-routing/evaluate";
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
        method: "patch",
        route_path: ROUTE_SETTINGS_AUTOPILOT,
        operation_id: "settingsAutopilotUpdate",
        summary: "Update autopilot profile settings for the authenticated user.",
        tag: "profile",
        secured: true,
        deprecated: false,
        success_status: "200",
        request_example: Some("settings_autopilot_update"),
        response_example: Some("settings_autopilot_update"),
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
        method: "get",
        route_path: ROUTE_AGENT_PAYMENTS_WALLET,
        operation_id: "agentPaymentsWallet",
        summary: "Read authenticated user's Spark wallet metadata.",
        tag: "payments",
        secured: true,
        deprecated: false,
        success_status: "200",
        request_example: None,
        response_example: Some("agent_payments_wallet"),
    },
    OpenApiContract {
        method: "post",
        route_path: ROUTE_AGENT_PAYMENTS_WALLET,
        operation_id: "agentPaymentsWalletUpsert",
        summary: "Create/import wallet for authenticated user.",
        tag: "payments",
        secured: true,
        deprecated: false,
        success_status: "200",
        request_example: Some("agent_payments_wallet_upsert"),
        response_example: Some("agent_payments_wallet_upsert"),
    },
    OpenApiContract {
        method: "get",
        route_path: ROUTE_AGENT_PAYMENTS_BALANCE,
        operation_id: "agentPaymentsBalance",
        summary: "Read authenticated wallet balance snapshot.",
        tag: "payments",
        secured: true,
        deprecated: false,
        success_status: "200",
        request_example: None,
        response_example: Some("agent_payments_balance"),
    },
    OpenApiContract {
        method: "post",
        route_path: ROUTE_AGENT_PAYMENTS_INVOICE,
        operation_id: "agentPaymentsInvoiceCreate",
        summary: "Create Lightning invoice from authenticated wallet.",
        tag: "payments",
        secured: true,
        deprecated: false,
        success_status: "200",
        request_example: Some("agent_payments_invoice_create"),
        response_example: Some("agent_payments_invoice"),
    },
    OpenApiContract {
        method: "post",
        route_path: ROUTE_AGENT_PAYMENTS_PAY,
        operation_id: "agentPaymentsInvoicePay",
        summary: "Pay a BOLT11 invoice from authenticated wallet.",
        tag: "payments",
        secured: true,
        deprecated: false,
        success_status: "200",
        request_example: Some("agent_payments_invoice_pay"),
        response_example: Some("agent_payments_payment"),
    },
    OpenApiContract {
        method: "post",
        route_path: ROUTE_AGENT_PAYMENTS_SEND_SPARK,
        operation_id: "agentPaymentsSendSpark",
        summary: "Send sats to Spark address from authenticated wallet.",
        tag: "payments",
        secured: true,
        deprecated: false,
        success_status: "200",
        request_example: Some("agent_payments_send_spark"),
        response_example: Some("agent_payments_transfer"),
    },
    OpenApiContract {
        method: "get",
        route_path: ROUTE_AGENTS_ME_WALLET,
        operation_id: "agentsMeWallet",
        summary: "Compatibility alias for agent-payments wallet read.",
        tag: "payments",
        secured: true,
        deprecated: true,
        success_status: "200",
        request_example: None,
        response_example: Some("agent_payments_wallet"),
    },
    OpenApiContract {
        method: "post",
        route_path: ROUTE_AGENTS_ME_WALLET,
        operation_id: "agentsMeWalletUpsert",
        summary: "Compatibility alias for agent-payments wallet upsert.",
        tag: "payments",
        secured: true,
        deprecated: true,
        success_status: "200",
        request_example: Some("agent_payments_wallet_upsert"),
        response_example: Some("agent_payments_wallet_upsert"),
    },
    OpenApiContract {
        method: "get",
        route_path: ROUTE_AGENTS_ME_BALANCE,
        operation_id: "agentsMeBalance",
        summary: "Compatibility alias for agent-payments balance read.",
        tag: "payments",
        secured: true,
        deprecated: true,
        success_status: "200",
        request_example: None,
        response_example: Some("agent_payments_balance"),
    },
    OpenApiContract {
        method: "post",
        route_path: ROUTE_PAYMENTS_INVOICE,
        operation_id: "paymentsInvoiceCreateAlias",
        summary: "Compatibility alias for agent-payments invoice create.",
        tag: "payments",
        secured: true,
        deprecated: true,
        success_status: "200",
        request_example: Some("agent_payments_invoice_create"),
        response_example: Some("agent_payments_invoice"),
    },
    OpenApiContract {
        method: "post",
        route_path: ROUTE_PAYMENTS_PAY,
        operation_id: "paymentsInvoicePayAlias",
        summary: "Compatibility alias for agent-payments invoice pay.",
        tag: "payments",
        secured: true,
        deprecated: true,
        success_status: "200",
        request_example: Some("agent_payments_invoice_pay"),
        response_example: Some("agent_payments_payment"),
    },
    OpenApiContract {
        method: "post",
        route_path: ROUTE_PAYMENTS_SEND_SPARK,
        operation_id: "paymentsSendSparkAlias",
        summary: "Compatibility alias for agent-payments spark transfer.",
        tag: "payments",
        secured: true,
        deprecated: true,
        success_status: "200",
        request_example: Some("agent_payments_send_spark"),
        response_example: Some("agent_payments_transfer"),
    },
    OpenApiContract {
        method: "get",
        route_path: ROUTE_SHOUTS,
        operation_id: "shoutsIndex",
        summary: "List public shouts with optional zone/cursor/date filters.",
        tag: "social",
        secured: false,
        deprecated: false,
        success_status: "200",
        request_example: None,
        response_example: Some("shouts_list"),
    },
    OpenApiContract {
        method: "post",
        route_path: ROUTE_SHOUTS,
        operation_id: "shoutsStore",
        summary: "Create a public shout (authenticated).",
        tag: "social",
        secured: true,
        deprecated: false,
        success_status: "201",
        request_example: Some("shout_create"),
        response_example: Some("shout"),
    },
    OpenApiContract {
        method: "get",
        route_path: ROUTE_SHOUTS_ZONES,
        operation_id: "shoutsZones",
        summary: "List top shout zones by 24h activity.",
        tag: "social",
        secured: false,
        deprecated: false,
        success_status: "200",
        request_example: None,
        response_example: Some("shouts_zones"),
    },
    OpenApiContract {
        method: "get",
        route_path: ROUTE_L402_WALLET,
        operation_id: "l402Wallet",
        summary: "Read L402 wallet summary, recent receipts, and spark wallet status.",
        tag: "l402",
        secured: true,
        deprecated: false,
        success_status: "200",
        request_example: None,
        response_example: Some("l402_wallet"),
    },
    OpenApiContract {
        method: "get",
        route_path: ROUTE_L402_TRANSACTIONS,
        operation_id: "l402Transactions",
        summary: "List L402 transaction receipts with pagination.",
        tag: "l402",
        secured: true,
        deprecated: false,
        success_status: "200",
        request_example: None,
        response_example: Some("l402_transactions"),
    },
    OpenApiContract {
        method: "get",
        route_path: ROUTE_L402_TRANSACTION_BY_ID,
        operation_id: "l402TransactionShow",
        summary: "Read one L402 transaction receipt by event id.",
        tag: "l402",
        secured: true,
        deprecated: false,
        success_status: "200",
        request_example: None,
        response_example: Some("l402_transaction"),
    },
    OpenApiContract {
        method: "get",
        route_path: ROUTE_L402_PAYWALLS,
        operation_id: "l402Paywalls",
        summary: "Read paywall aggregates grouped by host/scope.",
        tag: "l402",
        secured: true,
        deprecated: false,
        success_status: "200",
        request_example: None,
        response_example: Some("l402_paywalls"),
    },
    OpenApiContract {
        method: "post",
        route_path: ROUTE_L402_PAYWALLS,
        operation_id: "l402PaywallCreate",
        summary: "Create an L402 paywall rule (admin-only).",
        tag: "l402",
        secured: true,
        deprecated: false,
        success_status: "201",
        request_example: Some("l402_paywall_create"),
        response_example: Some("l402_paywall_create"),
    },
    OpenApiContract {
        method: "patch",
        route_path: ROUTE_L402_PAYWALL_BY_ID,
        operation_id: "l402PaywallUpdate",
        summary: "Update an L402 paywall rule (admin-only).",
        tag: "l402",
        secured: true,
        deprecated: false,
        success_status: "200",
        request_example: Some("l402_paywall_update"),
        response_example: Some("l402_paywall_update"),
    },
    OpenApiContract {
        method: "delete",
        route_path: ROUTE_L402_PAYWALL_BY_ID,
        operation_id: "l402PaywallDelete",
        summary: "Soft-delete an L402 paywall rule (admin-only).",
        tag: "l402",
        secured: true,
        deprecated: false,
        success_status: "200",
        request_example: None,
        response_example: Some("l402_paywall_delete"),
    },
    OpenApiContract {
        method: "get",
        route_path: ROUTE_L402_SETTLEMENTS,
        operation_id: "l402Settlements",
        summary: "Read paid settlement summaries and recent paid receipts.",
        tag: "l402",
        secured: true,
        deprecated: false,
        success_status: "200",
        request_example: None,
        response_example: Some("l402_settlements"),
    },
    OpenApiContract {
        method: "get",
        route_path: ROUTE_L402_DEPLOYMENTS,
        operation_id: "l402Deployments",
        summary: "Read L402 gateway/deployment operational events.",
        tag: "l402",
        secured: true,
        deprecated: false,
        success_status: "200",
        request_example: None,
        response_example: Some("l402_deployments"),
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
        method: "post",
        route_path: ROUTE_RUNTIME_INTERNAL_SECRET_FETCH,
        operation_id: "runtimeInternalSecretFetch",
        summary: "Fetch active integration credentials via signed runtime-internal headers.",
        tag: "runtime",
        secured: false,
        deprecated: false,
        success_status: "200",
        request_example: Some("runtime_internal_secret_fetch"),
        response_example: Some("runtime_internal_secret_fetch"),
    },
    OpenApiContract {
        method: "post",
        route_path: ROUTE_LIGHTNING_OPS_CONTROL_PLANE_QUERY,
        operation_id: "lightningOpsControlPlaneQuery",
        summary: "Dispatch lightning ops query functions guarded by shared ops secret.",
        tag: "l402",
        secured: false,
        deprecated: false,
        success_status: "200",
        request_example: Some("lightning_ops_control_plane_query"),
        response_example: Some("lightning_ops_control_plane_query"),
    },
    OpenApiContract {
        method: "post",
        route_path: ROUTE_LIGHTNING_OPS_CONTROL_PLANE_MUTATION,
        operation_id: "lightningOpsControlPlaneMutation",
        summary: "Dispatch lightning ops mutation functions guarded by shared ops secret.",
        tag: "l402",
        secured: false,
        deprecated: false,
        success_status: "200",
        request_example: Some("lightning_ops_control_plane_mutation"),
        response_example: Some("lightning_ops_control_plane_mutation"),
    },
    OpenApiContract {
        method: "post",
        route_path: ROUTE_RUNTIME_TOOLS_EXECUTE,
        operation_id: "runtimeToolsExecute",
        summary: "Execute typed runtime tool-pack operations with policy/replay receipts.",
        tag: "runtime",
        secured: true,
        deprecated: false,
        success_status: "200",
        request_example: Some("runtime_tools_execute"),
        response_example: Some("runtime_tools_execute"),
    },
    OpenApiContract {
        method: "get",
        route_path: ROUTE_RUNTIME_SKILLS_TOOL_SPECS,
        operation_id: "runtimeSkillToolSpecsList",
        summary: "List runtime tool specs (registry + built-ins).",
        tag: "runtime",
        secured: true,
        deprecated: false,
        success_status: "200",
        request_example: None,
        response_example: Some("runtime_skill_tool_specs_list"),
    },
    OpenApiContract {
        method: "post",
        route_path: ROUTE_RUNTIME_SKILLS_TOOL_SPECS,
        operation_id: "runtimeSkillToolSpecsStore",
        summary: "Upsert a runtime tool spec with schema/version validation.",
        tag: "runtime",
        secured: true,
        deprecated: false,
        success_status: "201",
        request_example: Some("runtime_skill_tool_spec_store"),
        response_example: Some("runtime_skill_tool_spec_store"),
    },
    OpenApiContract {
        method: "get",
        route_path: ROUTE_RUNTIME_SKILLS_SKILL_SPECS,
        operation_id: "runtimeSkillSpecsList",
        summary: "List runtime skill specs (registry + built-ins).",
        tag: "runtime",
        secured: true,
        deprecated: false,
        success_status: "200",
        request_example: None,
        response_example: Some("runtime_skill_specs_list"),
    },
    OpenApiContract {
        method: "post",
        route_path: ROUTE_RUNTIME_SKILLS_SKILL_SPECS,
        operation_id: "runtimeSkillSpecsStore",
        summary: "Upsert a runtime skill spec with schema/version validation.",
        tag: "runtime",
        secured: true,
        deprecated: false,
        success_status: "201",
        request_example: Some("runtime_skill_spec_store"),
        response_example: Some("runtime_skill_spec_store"),
    },
    OpenApiContract {
        method: "post",
        route_path: ROUTE_RUNTIME_SKILLS_SKILL_SPEC_PUBLISH,
        operation_id: "runtimeSkillSpecPublish",
        summary: "Publish a skill spec release bundle.",
        tag: "runtime",
        secured: true,
        deprecated: false,
        success_status: "201",
        request_example: None,
        response_example: Some("runtime_skill_spec_publish"),
    },
    OpenApiContract {
        method: "get",
        route_path: ROUTE_RUNTIME_SKILLS_RELEASE,
        operation_id: "runtimeSkillReleaseShow",
        summary: "Fetch a published runtime skill release bundle.",
        tag: "runtime",
        secured: true,
        deprecated: false,
        success_status: "200",
        request_example: None,
        response_example: Some("runtime_skill_release_show"),
    },
    OpenApiContract {
        method: "get",
        route_path: ROUTE_RUNTIME_CODEX_WORKERS,
        operation_id: "runtimeCodexWorkersList",
        summary: "List principal-owned Codex workers with lifecycle metadata.",
        tag: "codex",
        secured: true,
        deprecated: false,
        success_status: "200",
        request_example: None,
        response_example: Some("runtime_codex_workers_list"),
    },
    OpenApiContract {
        method: "post",
        route_path: ROUTE_RUNTIME_CODEX_WORKERS,
        operation_id: "runtimeCodexWorkersCreate",
        summary: "Create or reattach a Codex worker session.",
        tag: "codex",
        secured: true,
        deprecated: false,
        success_status: "202",
        request_example: Some("runtime_codex_worker_create"),
        response_example: Some("runtime_codex_worker_create"),
    },
    OpenApiContract {
        method: "get",
        route_path: ROUTE_RUNTIME_CODEX_WORKER_BY_ID,
        operation_id: "runtimeCodexWorkerShow",
        summary: "Read one principal-owned Codex worker snapshot.",
        tag: "codex",
        secured: true,
        deprecated: false,
        success_status: "200",
        request_example: None,
        response_example: Some("runtime_codex_worker_snapshot"),
    },
    OpenApiContract {
        method: "get",
        route_path: ROUTE_RUNTIME_CODEX_WORKER_STREAM,
        operation_id: "runtimeCodexWorkerStreamBootstrap",
        summary: "Bootstrap Codex worker live delivery via Khala WebSocket.",
        tag: "codex",
        secured: true,
        deprecated: false,
        success_status: "200",
        request_example: None,
        response_example: Some("runtime_codex_worker_stream_bootstrap"),
    },
    OpenApiContract {
        method: "post",
        route_path: ROUTE_RUNTIME_CODEX_WORKER_EVENTS,
        operation_id: "runtimeCodexWorkerEventsIngest",
        summary: "Ingest desktop-originated worker events into durable log.",
        tag: "codex",
        secured: true,
        deprecated: false,
        success_status: "202",
        request_example: Some("runtime_codex_worker_events"),
        response_example: Some("runtime_codex_worker_events"),
    },
    OpenApiContract {
        method: "post",
        route_path: ROUTE_RUNTIME_CODEX_WORKER_STOP,
        operation_id: "runtimeCodexWorkerStop",
        summary: "Request graceful worker stop and durable terminal state.",
        tag: "codex",
        secured: true,
        deprecated: false,
        success_status: "202",
        request_example: Some("runtime_codex_worker_stop"),
        response_example: Some("runtime_codex_worker_stop"),
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
        method: "get",
        route_path: ROUTE_V1_CONTROL_RUNTIME_ROUTING_STATUS,
        operation_id: "controlRuntimeRoutingStatus",
        summary: "Read runtime routing status and override records.",
        tag: "control",
        secured: true,
        deprecated: false,
        success_status: "200",
        request_example: None,
        response_example: Some("runtime_routing_status"),
    },
    OpenApiContract {
        method: "post",
        route_path: ROUTE_V1_CONTROL_RUNTIME_ROUTING_OVERRIDE,
        operation_id: "controlRuntimeRoutingOverride",
        summary: "Upsert runtime driver override record.",
        tag: "control",
        secured: true,
        deprecated: false,
        success_status: "200",
        request_example: Some("runtime_routing_override"),
        response_example: Some("runtime_routing_override"),
    },
    OpenApiContract {
        method: "post",
        route_path: ROUTE_V1_CONTROL_RUNTIME_ROUTING_EVALUATE,
        operation_id: "controlRuntimeRoutingEvaluate",
        summary: "Evaluate runtime driver decision for a user/thread.",
        tag: "control",
        secured: true,
        deprecated: false,
        success_status: "200",
        request_example: Some("runtime_routing_evaluate"),
        response_example: Some("runtime_routing_evaluate"),
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
        "settings_autopilot_update" => Some(json!({
            "displayName": "Chris Autopilot",
            "tagline": "Persistent and practical",
            "ownerDisplayName": "Chris",
            "personaSummary": "Keep it concise and engineering-minded.",
            "autopilotVoice": "calm and direct",
            "principlesText": "Prefer verification over guessing\nAsk before irreversible actions"
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
        "agent_payments_wallet_upsert" => Some(json!({
            "mnemonic": "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about"
        })),
        "agent_payments_invoice_create" => Some(json!({
            "amountSats": 42,
            "description": "OpenAgents test invoice"
        })),
        "agent_payments_invoice_pay" => Some(json!({
            "invoice": "lnbc42n1p0openagentssampleinvoice00000000000000",
            "maxAmountSats": 42,
            "timeoutMs": 12000,
            "host": "sats4ai.com"
        })),
        "agent_payments_send_spark" => Some(json!({
            "sparkAddress": "spark:recipient",
            "amountSats": 21,
            "timeoutMs": 12000
        })),
        "shout_create" => Some(json!({
            "body": "L402 payment shipped",
            "zone": "l402"
        })),
        "l402_paywall_create" => Some(json!({
            "name": "Default",
            "hostRegexp": "sats4ai\\.com",
            "pathRegexp": "^/api/.*",
            "priceMsats": 1000,
            "upstream": "https://upstream.openagents.com",
            "enabled": true,
            "metadata": {
                "tier": "default"
            }
        })),
        "l402_paywall_update" => Some(json!({
            "priceMsats": 2500,
            "enabled": false,
            "metadata": {
                "tier": "burst"
            }
        })),
        "sync_token" => Some(json!({
            "scopes": ["runtime.codex_worker_events"],
            "topics": ["org:openagents:worker_events"],
            "ttl_seconds": 300,
            "device_id": "ios:device"
        })),
        "runtime_internal_secret_fetch" => Some(json!({
            "user_id": 42,
            "provider": "resend",
            "integration_id": "resend.primary",
            "run_id": "run_123",
            "tool_call_id": "tool_123",
            "org_id": "org_abc"
        })),
        "lightning_ops_control_plane_query" => Some(json!({
            "functionName": "lightning/ops:listPaywallControlPlaneState",
            "args": {
                "secret": "ops-secret",
                "statuses": ["active", "paused"]
            }
        })),
        "lightning_ops_control_plane_mutation" => Some(json!({
            "functionName": "lightning/security:setGlobalPause",
            "args": {
                "secret": "ops-secret",
                "active": true,
                "reason": "Emergency pause",
                "updatedBy": "ops@openagents.com"
            }
        })),
        "thread_message" => Some(json!({ "text": "Summarize this thread." })),
        "runtime_tools_execute" => Some(json!({
            "tool_pack": "coding.v1",
            "mode": "replay",
            "run_id": "run_tools_1",
            "thread_id": "thread_tools_1",
            "manifest_ref": {"integration_id": "github.primary"},
            "request": {
                "integration_id": "github.primary",
                "operation": "get_issue",
                "repository": "OpenAgentsInc/openagents",
                "issue_number": 1747,
                "tool_call_id": "tool_call_001"
            },
            "policy": {
                "authorization_id": "auth_123",
                "authorization_mode": "delegated_budget",
                "budget": {"max_per_call_sats": 100}
            }
        })),
        "runtime_skill_tool_spec_store" => Some(json!({
            "state": "validated",
            "tool_spec": {
                "tool_id": "github.custom",
                "version": 1,
                "tool_pack": "coding.v1",
                "name": "GitHub Custom",
                "execution_kind": "http",
                "integration_manifest": {
                    "manifest_version": "coding.integration.v1",
                    "integration_id": "github.custom",
                    "provider": "github",
                    "status": "active",
                    "tool_pack": "coding.v1",
                    "capabilities": ["get_issue", "get_pull_request"]
                }
            }
        })),
        "runtime_skill_spec_store" => Some(json!({
            "state": "validated",
            "skill_spec": {
                "skill_id": "github-coding-custom",
                "version": 1,
                "name": "GitHub Coding Custom",
                "allowed_tools": [{"tool_id": "github.custom", "version": 1}],
                "compatibility": {"runtime": "runtime"}
            }
        })),
        "runtime_codex_worker_create" => Some(json!({
            "worker_id": "codexw_12345",
            "workspace_ref": "workspace://demo",
            "codex_home_ref": "codex-home://demo",
            "adapter": "in_memory",
            "metadata": {"surface": "web"}
        })),
        "runtime_codex_worker_events" => Some(json!({
            "event": {
                "event_type": "worker.event",
                "payload": {
                    "source": "autopilot-ios",
                    "method": "ios/handshake",
                    "handshake_id": "hs_123",
                    "device_id": "device_abc",
                    "occurred_at": "2026-02-22T00:00:00Z"
                }
            }
        })),
        "runtime_codex_worker_stop" => Some(json!({
            "reason": "done"
        })),
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
        "runtime_routing_override" => Some(json!({
            "scope_type": "user",
            "scope_id": "usr_123",
            "driver": "elixir",
            "is_active": true,
            "reason": "canary cohort"
        })),
        "runtime_routing_evaluate" => Some(json!({
            "thread_id": "thread_123",
            "user_id": "usr_123"
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
                "promptContext": "autopilot_id=ap_123\nconfig_version=2\nhandle=ep212-bot\npersona_summary=Pragmatic and concise",
                "toolPolicy": {
                    "policyApplied": true,
                    "authRestricted": false,
                    "sessionAuthenticated": true,
                    "autopilotId": "ap_123",
                    "availableTools": [
                        "openagents_api",
                        "lightning_l402_fetch",
                        "lightning_l402_approve"
                    ],
                    "exposedTools": ["openagents_api"],
                    "allowlist": ["openagents_api", "lightning_l402_fetch"],
                    "denylist": ["lightning_l402_fetch"],
                    "removedByAllowlist": ["lightning_l402_approve"],
                    "removedByDenylist": ["lightning_l402_fetch"],
                    "removedByAuthGate": []
                },
                "runtimeBinding": {
                    "id": "arb_123",
                    "runtimeType": "runtime",
                    "runtimeRef": "desktopw:autopilot",
                    "isPrimary": true,
                    "driverHint": "elixir",
                    "lastSeenAt": "2026-02-22T00:00:00Z",
                    "meta": {"region": "us-central1"},
                    "createdAt": "2026-02-22T00:00:00Z",
                    "updatedAt": "2026-02-22T00:00:00Z"
                },
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
        "settings_autopilot_update" => Some(json!({
            "data": {
                "status": "autopilot-updated",
                "autopilot": {
                    "id": "ap_123",
                    "handle": "ep212-bot",
                    "displayName": "EP212 Bot",
                    "status": "active",
                    "visibility": "private",
                    "ownerUserId": "usr_123",
                    "avatar": null,
                    "tagline": "Persistent and practical",
                    "configVersion": 2,
                    "profile": {
                        "ownerDisplayName": "Chris",
                        "personaSummary": "Pragmatic and concise",
                        "autopilotVoice": "calm and direct",
                        "principles": ["Prefer verification over guessing", "Ask before irreversible actions"],
                        "preferences": [],
                        "onboardingAnswers": [],
                        "schemaVersion": 1
                    },
                    "policy": {
                        "modelProvider": null,
                        "model": null,
                        "toolAllowlist": [],
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
        "agent_payments_wallet" => Some(json!({
            "data": {
                "wallet": {
                    "id": "wallet_123",
                    "walletId": "wallet_123",
                    "sparkAddress": "usr_123@spark.openagents.local",
                    "lightningAddress": "usr_123@openagents.local",
                    "identityPubkey": "pubkey_123",
                    "balanceSats": 4200,
                    "status": "active",
                    "provider": "spark_executor",
                    "lastError": null,
                    "lastSyncedAt": "2026-02-22T00:00:00Z",
                    "createdAt": "2026-02-22T00:00:00Z",
                    "updatedAt": "2026-02-22T00:00:00Z"
                }
            }
        })),
        "agent_payments_wallet_upsert" => Some(json!({
            "data": {
                "wallet": {
                    "id": "wallet_123",
                    "walletId": "wallet_123",
                    "sparkAddress": "usr_123@spark.openagents.local",
                    "lightningAddress": "usr_123@openagents.local",
                    "identityPubkey": "pubkey_123",
                    "balanceSats": 0,
                    "status": "active",
                    "provider": "spark_executor",
                    "lastError": null,
                    "lastSyncedAt": "2026-02-22T00:00:00Z",
                    "createdAt": "2026-02-22T00:00:00Z",
                    "updatedAt": "2026-02-22T00:00:00Z"
                },
                "action": "ensured"
            }
        })),
        "agent_payments_balance" => Some(json!({
            "data": {
                "walletId": "wallet_123",
                "balanceSats": 4200,
                "sparkAddress": "usr_123@spark.openagents.local",
                "lightningAddress": "usr_123@openagents.local",
                "lastSyncedAt": "2026-02-22T00:00:00Z"
            }
        })),
        "agent_payments_invoice" => Some(json!({
            "data": {
                "invoice": {
                    "paymentRequest": "lnbc42n1psampleinvoice",
                    "amountSats": 42,
                    "description": "OpenAgents test invoice",
                    "expiresAt": "2026-02-22T00:15:00Z",
                    "raw": {
                        "paymentRequest": "lnbc42n1psampleinvoice",
                        "amountSats": 42
                    }
                }
            }
        })),
        "agent_payments_payment" => Some(json!({
            "data": {
                "payment": {
                    "paymentId": "payment_123",
                    "preimage": "abc123",
                    "proofReference": "preimage:abc123",
                    "quotedAmountMsats": 42000,
                    "maxAmountMsats": 42000,
                    "status": "completed",
                    "raw": {
                        "paymentId": "payment_123",
                        "status": "completed"
                    }
                }
            }
        })),
        "agent_payments_transfer" => Some(json!({
            "data": {
                "transfer": {
                    "sparkAddress": "spark:recipient",
                    "amountSats": 21,
                    "status": "completed",
                    "paymentId": "spark_123",
                    "raw": {
                        "sparkAddress": "spark:recipient",
                        "amountSats": 21
                    }
                }
            }
        })),
        "shout" => Some(json!({
            "data": {
                "id": 42,
                "zone": "l402",
                "body": "L402 payment shipped",
                "visibility": "public",
                "author": {
                    "id": "usr_123",
                    "name": "OpenAgents User",
                    "handle": "openagents-user",
                    "avatar": null
                },
                "createdAt": "2026-02-22T00:00:00Z",
                "updatedAt": "2026-02-22T00:00:00Z"
            }
        })),
        "shouts_list" => Some(json!({
            "data": [
                {
                    "id": 42,
                    "zone": "l402",
                    "body": "L402 payment shipped",
                    "visibility": "public",
                    "author": {
                        "id": "usr_123",
                        "name": "OpenAgents User",
                        "handle": "openagents-user",
                        "avatar": null
                    },
                    "createdAt": "2026-02-22T00:00:00Z",
                    "updatedAt": "2026-02-22T00:00:00Z"
                }
            ],
            "meta": {
                "nextCursor": "42"
            }
        })),
        "shouts_zones" => Some(json!({
            "data": [
                {"zone": "global", "count24h": 12},
                {"zone": "l402", "count24h": 4}
            ]
        })),
        "l402_wallet" => Some(json!({
            "data": {
                "summary": {
                    "totalAttempts": 3,
                    "paidCount": 1,
                    "cachedCount": 1,
                    "blockedCount": 1,
                    "failedCount": 0,
                    "totalPaidMsats": 2100,
                    "totalPaidSats": 2.1
                },
                "lastPaid": {
                    "eventId": 14,
                    "threadId": "thread_1",
                    "runId": "run_1",
                    "status": "paid",
                    "host": "sats4ai.com",
                    "paid": true,
                    "amountMsats": 2100,
                    "amountSats": 2.1
                },
                "recent": [],
                "sparkWallet": {
                    "walletId": "wallet_123",
                    "sparkAddress": "spark:abc",
                    "lightningAddress": "ln@openagents.com",
                    "identityPubkey": "pubkey_1",
                    "balanceSats": 4200,
                    "status": "active",
                    "provider": "spark_executor",
                    "lastError": null,
                    "lastSyncedAt": "2026-02-22T00:00:00Z"
                },
                "settings": {
                    "enforceHostAllowlist": false,
                    "allowlistHosts": ["sats4ai.com", "l402.openagents.com"],
                    "invoicePayer": "spark_wallet",
                    "credentialTtlSeconds": 600,
                    "paymentTimeoutMs": 12000,
                    "responseMaxBytes": 65536,
                    "responsePreviewBytes": 1024
                },
                "filter": {
                    "autopilot": {
                        "id": "ap_123",
                        "handle": "payments-bot"
                    }
                }
            }
        })),
        "l402_transactions" => Some(json!({
            "data": {
                "transactions": [
                    {
                        "eventId": 14,
                        "threadId": "thread_1",
                        "threadTitle": "Conversation 1",
                        "runId": "run_1",
                        "runStatus": "completed",
                        "createdAt": "2026-02-22T00:00:00Z",
                        "status": "paid",
                        "host": "sats4ai.com",
                        "scope": "fetch",
                        "paid": true,
                        "cacheHit": false,
                        "cacheStatus": null,
                        "amountMsats": 2100,
                        "amountSats": 2.1
                    }
                ],
                "pagination": {
                    "currentPage": 1,
                    "lastPage": 1,
                    "perPage": 30,
                    "total": 1,
                    "hasMorePages": false
                },
                "filter": {
                    "autopilot": {
                        "id": "ap_123",
                        "handle": "payments-bot"
                    }
                }
            }
        })),
        "l402_transaction" => Some(json!({
            "data": {
                "transaction": {
                    "eventId": 14,
                    "threadId": "thread_1",
                    "threadTitle": "Conversation 1",
                    "runId": "run_1",
                    "runStatus": "completed",
                    "createdAt": "2026-02-22T00:00:00Z",
                    "status": "paid",
                    "host": "sats4ai.com",
                    "scope": "fetch",
                    "paid": true,
                    "cacheHit": false,
                    "cacheStatus": null,
                    "amountMsats": 2100,
                    "amountSats": 2.1
                }
            }
        })),
        "l402_paywalls" => Some(json!({
            "data": {
                "paywalls": [
                    {
                        "host": "sats4ai.com",
                        "scope": "fetch",
                        "attempts": 2,
                        "paid": 1,
                        "cached": 1,
                        "blocked": 0,
                        "failed": 0,
                        "totalPaidMsats": 2100,
                        "totalPaidSats": 2.1,
                        "lastAttemptAt": "2026-02-22T00:00:00Z",
                        "lastStatus": "cached"
                    }
                ],
                "summary": {
                    "uniqueTargets": 1,
                    "totalAttempts": 2,
                    "totalPaidCount": 1
                },
                "filter": {
                    "autopilot": null
                }
            }
        })),
        "l402_paywall_create" => Some(json!({
            "data": {
                "paywall": {
                    "id": "pw_123",
                    "ownerUserId": "usr_123",
                    "name": "Default",
                    "hostRegexp": "sats4ai\\.com",
                    "pathRegexp": "^/api/.*",
                    "priceMsats": 1000,
                    "upstream": "https://upstream.openagents.com",
                    "enabled": true,
                    "metadata": {"tier": "default"},
                    "lastReconcileStatus": null,
                    "lastReconcileError": null,
                    "lastReconciledAt": null,
                    "createdAt": "2026-02-22T00:00:00Z",
                    "updatedAt": "2026-02-22T00:00:00Z",
                    "deletedAt": null
                },
                "deployment": {
                    "status": "applied",
                    "eventType": "l402_paywall_created",
                    "eventId": 101,
                    "reverted": false
                },
                "mutationEventId": 101
            }
        })),
        "l402_paywall_update" => Some(json!({
            "data": {
                "paywall": {
                    "id": "pw_123",
                    "ownerUserId": "usr_123",
                    "name": "Default",
                    "hostRegexp": "sats4ai\\.com",
                    "pathRegexp": "^/api/.*",
                    "priceMsats": 2500,
                    "upstream": "https://upstream.openagents.com",
                    "enabled": false,
                    "metadata": {"tier": "burst"},
                    "lastReconcileStatus": "applied",
                    "lastReconcileError": null,
                    "lastReconciledAt": "2026-02-22T00:00:00Z",
                    "createdAt": "2026-02-22T00:00:00Z",
                    "updatedAt": "2026-02-22T00:05:00Z",
                    "deletedAt": null
                },
                "deployment": {
                    "status": "applied",
                    "eventType": "l402_paywall_updated",
                    "eventId": 102,
                    "reverted": false
                },
                "mutationEventId": 102
            }
        })),
        "l402_paywall_delete" => Some(json!({
            "data": {
                "deleted": true,
                "paywall": {
                    "id": "pw_123",
                    "ownerUserId": "usr_123",
                    "name": "Default",
                    "hostRegexp": "sats4ai\\.com",
                    "pathRegexp": "^/api/.*",
                    "priceMsats": 2500,
                    "upstream": "https://upstream.openagents.com",
                    "enabled": false,
                    "metadata": {"tier": "burst"},
                    "lastReconcileStatus": "applied",
                    "lastReconcileError": null,
                    "lastReconciledAt": "2026-02-22T00:05:00Z",
                    "createdAt": "2026-02-22T00:00:00Z",
                    "updatedAt": "2026-02-22T00:10:00Z",
                    "deletedAt": "2026-02-22T00:10:00Z"
                },
                "deployment": {
                    "status": "applied",
                    "eventType": "l402_paywall_deleted",
                    "eventId": 103,
                    "reverted": false
                },
                "mutationEventId": 103
            }
        })),
        "l402_settlements" => Some(json!({
            "data": {
                "summary": {
                    "settledCount": 1,
                    "totalMsats": 2100,
                    "totalSats": 2.1,
                    "latestSettlementAt": "2026-02-22T00:00:00Z"
                },
                "daily": [
                    {
                        "date": "2026-02-22",
                        "count": 1,
                        "totalMsats": 2100,
                        "totalSats": 2.1
                    }
                ],
                "settlements": [],
                "filter": {
                    "autopilot": null
                }
            }
        })),
        "l402_deployments" => Some(json!({
            "data": {
                "deployments": [
                    {
                        "eventId": 91,
                        "type": "l402_gateway_event",
                        "createdAt": "2026-02-22T00:00:00Z",
                        "payload": {
                            "status": "ok"
                        }
                    }
                ],
                "configSnapshot": {
                    "enforceHostAllowlist": false,
                    "allowlistHosts": ["sats4ai.com", "l402.openagents.com"],
                    "invoicePayer": "spark_wallet",
                    "credentialTtlSeconds": 600,
                    "paymentTimeoutMs": 12000,
                    "demoPresets": ["sats4ai", "ep212_openagents_premium", "ep212_openagents_expensive", "fake"]
                },
                "filter": {
                    "autopilot": null
                }
            }
        })),
        "sync_token" => Some(json!({
            "data": {
                "token": "eyJ...",
                "token_type": "Bearer",
                "expires_at": "2026-02-22T00:00:00Z"
            }
        })),
        "runtime_internal_secret_fetch" => Some(json!({
            "data": {
                "provider": "resend",
                "secret": "re_live_1234567890",
                "cache_ttl_ms": 60000,
                "scope": {
                    "user_id": 42,
                    "provider": "resend",
                    "integration_id": "resend.primary",
                    "run_id": "run_123",
                    "tool_call_id": "tool_123",
                    "org_id": "org_abc"
                },
                "fetched_at": "2026-02-22T00:00:00Z"
            }
        })),
        "lightning_ops_control_plane_query" => Some(json!({
            "ok": true,
            "paywalls": [
                {
                    "paywallId": "pw_123",
                    "ownerId": "owner_usr_123",
                    "name": "Default",
                    "status": "active",
                    "createdAtMs": 1700000000000i64,
                    "updatedAtMs": 1700000000000i64,
                    "policy": {
                        "paywallId": "pw_123",
                        "ownerId": "owner_usr_123",
                        "pricingMode": "fixed",
                        "fixedAmountMsats": 1000,
                        "killSwitch": false,
                        "createdAtMs": 1700000000000i64,
                        "updatedAtMs": 1700000000000i64
                    },
                    "routes": [
                        {
                            "routeId": "route_pw_123",
                            "paywallId": "pw_123",
                            "ownerId": "owner_usr_123",
                            "hostPattern": "sats4ai\\.com",
                            "pathPattern": "^/api/.*",
                            "upstreamUrl": "https://upstream.openagents.com",
                            "protocol": "https",
                            "timeoutMs": 6000,
                            "priority": 10,
                            "createdAtMs": 1700000000000i64,
                            "updatedAtMs": 1700000000000i64
                        }
                    ]
                }
            ]
        })),
        "lightning_ops_control_plane_mutation" => Some(json!({
            "ok": true,
            "global": {
                "stateId": "global",
                "globalPause": true,
                "denyReasonCode": "global_pause_active",
                "denyReason": "Emergency pause",
                "updatedBy": "ops@openagents.com",
                "updatedAtMs": 1700000000000i64
            }
        })),
        "thread_message" => Some(json!({
            "data": {
                "status": "accepted",
                "thread_id": "thread-1"
            }
        })),
        "runtime_tools_execute" => Some(json!({
            "data": {
                "state": "succeeded",
                "decision": "allowed",
                "reason_code": "policy_allowed.default",
                "tool_pack": "coding.v1",
                "mode": "replay",
                "idempotentReplay": false,
                "receipt": {
                    "receipt_id": "coding_8b5f2caa1122334455667788",
                    "replay_hash": "sha256:8b5f2caa11223344556677889900aabbccddeeff00112233445566778899aabb"
                },
                "policy": {
                    "writeApproved": false,
                    "writeOperationsMode": "enforce",
                    "maxPerCallSats": 100,
                    "operationCostSats": 5
                },
                "request": {
                    "integration_id": "github.primary",
                    "operation": "get_issue",
                    "repository": "OpenAgentsInc/openagents",
                    "issue_number": 1747,
                    "pull_number": null,
                    "tool_call_id": "tool_call_001",
                    "run_id": "run_tools_1",
                    "thread_id": "thread_tools_1",
                    "user_id": 42
                },
                "result": {
                    "integration_id": "github.primary",
                    "operation": "get_issue",
                    "repository": "OpenAgentsInc/openagents",
                    "issue_number": 1747,
                    "issue": {
                        "number": 1747,
                        "title": "Issue #1747",
                        "url": "https://github.com/OpenAgentsInc/openagents/issues/1747"
                    }
                }
            }
        })),
        "runtime_skill_tool_specs_list" => Some(json!({
            "data": [
                {
                    "tool_id": "github.primary",
                    "version": 1,
                    "tool_pack": "coding.v1",
                    "state": "published"
                }
            ]
        })),
        "runtime_skill_tool_spec_store" => Some(json!({
            "data": {
                "tool_id": "github.custom",
                "version": 1,
                "tool_pack": "coding.v1",
                "state": "validated"
            }
        })),
        "runtime_skill_specs_list" => Some(json!({
            "data": [
                {
                    "skill_id": "github-coding",
                    "version": 1,
                    "state": "published"
                }
            ]
        })),
        "runtime_skill_spec_store" => Some(json!({
            "data": {
                "skill_id": "github-coding-custom",
                "version": 1,
                "state": "validated"
            }
        })),
        "runtime_skill_spec_publish" => Some(json!({
            "data": {
                "release_id": "skillrel_abc123",
                "skill_id": "github-coding-custom",
                "version": 1,
                "bundle_hash": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                "published_at": "2026-02-22T00:00:00Z"
            }
        })),
        "runtime_skill_release_show" => Some(json!({
            "data": {
                "release_id": "skillrel_abc123",
                "skill_id": "github-coding-custom",
                "version": 1,
                "bundle_hash": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
                "published_at": "2026-02-22T00:00:00Z",
                "bundle": {
                    "bundle_format": "agent_skills.v1",
                    "skill_spec": {
                        "skill_id": "github-coding-custom",
                        "version": 1
                    }
                }
            }
        })),
        "runtime_codex_workers_list" => Some(json!({
            "data": [
                {
                    "worker_id": "codexw_12345",
                    "status": "running",
                    "latest_seq": 12,
                    "workspace_ref": "workspace://demo",
                    "heartbeat_state": "fresh",
                    "heartbeat_age_ms": 843,
                    "heartbeat_stale_after_ms": 120000,
                    "khala_projection": {
                        "status": "in_sync",
                        "lag_events": 0,
                        "last_runtime_seq": 12,
                        "last_projected_at": "2026-02-22T00:00:00Z"
                    }
                }
            ]
        })),
        "runtime_codex_worker_create" => Some(json!({
            "data": {
                "workerId": "codexw_12345",
                "status": "running",
                "latestSeq": 0,
                "idempotentReplay": false
            }
        })),
        "runtime_codex_worker_snapshot" => Some(json!({
            "data": {
                "worker_id": "codexw_12345",
                "status": "running",
                "latest_seq": 12,
                "heartbeat_state": "fresh",
                "heartbeat_age_ms": 843,
                "heartbeat_stale_after_ms": 120000
            }
        })),
        "runtime_codex_worker_stream_bootstrap" => Some(json!({
            "data": {
                "worker_id": "codexw_12345",
                "stream_protocol": "khala_ws",
                "cursor": 0,
                "tail_ms": 15000,
                "delivery": {
                    "transport": "khala_ws",
                    "topic": "org:openagents:worker_events",
                    "scope": "runtime.codex_worker_events",
                    "syncTokenRoute": "/api/sync/token"
                },
                "events": []
            }
        })),
        "runtime_codex_worker_events" => Some(json!({
            "data": {
                "worker_id": "codexw_12345",
                "seq": 13,
                "event_type": "worker.event",
                "payload": {"method": "turn/started"},
                "occurred_at": "2026-02-22T00:00:00Z"
            }
        })),
        "runtime_codex_worker_stop" => Some(json!({
            "data": {
                "worker_id": "codexw_12345",
                "status": "stopped",
                "seq": 14,
                "idempotent_replay": false
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
                "service": "openagents-control-service",
                "authProvider": "mock",
                "activeOrgId": "org:openagents",
                "compatibility": {
                    "protocolVersion": "openagents.control.v1",
                    "minClientBuildId": "00000000T000000Z",
                    "maxClientBuildId": null,
                    "minSchemaVersion": 1,
                    "maxSchemaVersion": 1
                },
                "routeSplit": {
                    "mode": "cohort",
                    "override_target": null
                },
                "runtimeRouting": {
                    "default_driver": "legacy",
                    "forced_driver": null,
                    "force_legacy": false
                }
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
        "runtime_routing_status" => Some(json!({
            "data": {
                "default_driver": "legacy",
                "forced_driver": null,
                "force_legacy": false,
                "overrides_enabled": true,
                "canary_user_percent": 0,
                "canary_autopilot_percent": 0,
                "shadow": {
                    "enabled": false,
                    "sample_rate": 1.0,
                    "max_capture_bytes": 200000
                },
                "overrides": [
                    {
                        "scope_type": "user",
                        "scope_id": "usr_123",
                        "driver": "elixir",
                        "is_active": true
                    }
                ]
            }
        })),
        "runtime_routing_override" => Some(json!({
            "data": {
                "override": {
                    "scope_type": "user",
                    "scope_id": "usr_123",
                    "driver": "elixir",
                    "is_active": true
                },
                "status": {
                    "default_driver": "legacy",
                    "forced_driver": null,
                    "force_legacy": false
                }
            }
        })),
        "runtime_routing_evaluate" => Some(json!({
            "data": {
                "driver": "elixir",
                "reason": "user_override",
                "default_driver": "legacy",
                "shadow": {
                    "enabled": false,
                    "mirrored": false,
                    "sample_rate": 1.0,
                    "shadow_driver": null
                }
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
