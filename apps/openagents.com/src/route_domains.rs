use axum::{
    middleware,
    routing::{delete, get, patch, post},
    Router,
};

use super::*;

pub(super) fn build_public_api_router(
    runtime_internal_secret_fetch_path: &str,
    runtime_internal_state: AppState,
    auth_email_throttle_state: AppState,
    auth_register_throttle_state: AppState,
) -> Router<AppState> {
    Router::<AppState>::new()
        .route(
            ROUTE_LIGHTNING_OPS_CONTROL_PLANE_QUERY,
            post(lightning_ops_control_plane_query),
        )
        .route(
            ROUTE_LIGHTNING_OPS_CONTROL_PLANE_MUTATION,
            post(lightning_ops_control_plane_mutation),
        )
        .route(
            runtime_internal_secret_fetch_path,
            post(runtime_internal_secret_fetch).route_layer(middleware::from_fn_with_state(
                runtime_internal_state,
                runtime_internal_request_gate,
            )),
        )
        .route(
            ROUTE_AUTH_EMAIL,
            post(send_email_code).route_layer(middleware::from_fn_with_state(
                auth_email_throttle_state,
                throttle_auth_email_gate,
            )),
        )
        .route(
            ROUTE_AUTH_REGISTER,
            post(auth_register).route_layer(middleware::from_fn_with_state(
                auth_register_throttle_state,
                throttle_auth_email_gate,
            )),
        )
        .route(ROUTE_AUTH_VERIFY, post(verify_email_code))
        .route(ROUTE_AUTH_REFRESH, post(refresh_session))
        .route(ROUTE_SHOUTS, get(shouts_index))
        .route(ROUTE_SHOUTS_ZONES, get(shouts_zones))
        .route(ROUTE_SMOKE_STREAM, get(smoke_stream))
        .route(ROUTE_WEBHOOKS_RESEND, post(webhooks_resend_store))
}

pub(super) fn build_protected_api_router(
    thread_message_throttle_state: AppState,
    codex_control_request_throttle_state: AppState,
    admin_state: AppState,
    workos_session_state: AppState,
    authenticated_routes_state: AppState,
) -> Router<AppState> {
    let router = Router::<AppState>::new();
    let router = add_auth_identity_routes(router);
    let router = add_autopilot_routes(router);
    let router = add_settings_inbox_token_routes(router);
    let router = add_org_sync_social_routes(router);
    let router = add_payments_l402_routes(router);
    let router = add_legacy_compat_routes(router);
    let router = add_runtime_control_routes(
        router,
        thread_message_throttle_state,
        codex_control_request_throttle_state,
    );
    let router = add_admin_control_routes(router, admin_state);
    router
        .route_layer(middleware::from_fn_with_state(
            workos_session_state,
            workos_session_gate,
        ))
        .route_layer(middleware::from_fn_with_state(
            authenticated_routes_state,
            auth_session_gate,
        ))
}

fn add_auth_identity_routes(router: Router<AppState>) -> Router<AppState> {
    router
        .route(ROUTE_AUTH_SESSION, get(current_session))
        .route(ROUTE_AUTH_SESSIONS, get(list_sessions))
        .route(ROUTE_AUTH_SESSIONS_REVOKE, post(revoke_sessions))
        .route(ROUTE_AUTH_LOGOUT, post(logout_session))
        .route(ROUTE_ME, get(me))
        .route(ROUTE_V1_AUTH_SESSION, get(current_session))
        .route(ROUTE_V1_AUTH_SESSIONS, get(list_sessions))
        .route(ROUTE_V1_AUTH_SESSIONS_REVOKE, post(revoke_sessions))
}

fn add_autopilot_routes(router: Router<AppState>) -> Router<AppState> {
    router
        .route(
            ROUTE_AUTOPILOTS,
            get(list_autopilots).post(create_autopilot),
        )
        .route(
            ROUTE_AUTOPILOTS_BY_ID,
            get(show_autopilot).patch(update_autopilot),
        )
        .route(
            ROUTE_AUTOPILOTS_THREADS,
            get(list_autopilot_threads).post(create_autopilot_thread),
        )
        .route(ROUTE_AUTOPILOTS_STREAM, post(autopilot_stream))
}

fn add_settings_inbox_token_routes(router: Router<AppState>) -> Router<AppState> {
    router
        .route(
            ROUTE_TOKENS,
            get(list_personal_access_tokens)
                .post(create_personal_access_token)
                .delete(delete_all_personal_access_tokens),
        )
        .route(
            ROUTE_SETTINGS_PROFILE,
            get(settings_profile_show)
                .patch(settings_profile_update)
                .delete(settings_profile_delete),
        )
        .route(ROUTE_SETTINGS_AUTOPILOT, patch(settings_autopilot_update))
        .route(
            ROUTE_SETTINGS_INTEGRATIONS_RESEND,
            post(settings_integrations_resend_upsert)
                .delete(settings_integrations_resend_disconnect),
        )
        .route(
            ROUTE_SETTINGS_INTEGRATIONS_RESEND_TEST,
            post(settings_integrations_resend_test),
        )
        .route(
            ROUTE_SETTINGS_INTEGRATIONS_GOOGLE_REDIRECT,
            get(settings_integrations_google_redirect),
        )
        .route(
            ROUTE_SETTINGS_INTEGRATIONS_GOOGLE_CALLBACK,
            get(settings_integrations_google_callback),
        )
        .route(
            ROUTE_SETTINGS_INTEGRATIONS_GOOGLE,
            delete(settings_integrations_google_disconnect),
        )
        .route(ROUTE_INBOX_THREADS, get(inbox_threads_index))
        .route(ROUTE_INBOX_REFRESH, post(inbox_refresh))
        .route(ROUTE_INBOX_THREAD_DETAIL, get(inbox_thread_detail))
        .route(ROUTE_INBOX_THREAD_APPROVE, post(inbox_thread_approve))
        .route(ROUTE_INBOX_THREAD_REJECT, post(inbox_thread_reject))
        .route(ROUTE_INBOX_REPLY_SEND, post(inbox_thread_reply_send))
        .route(
            ROUTE_TOKENS_CURRENT,
            delete(delete_current_personal_access_token),
        )
        .route(ROUTE_TOKENS_BY_ID, delete(delete_personal_access_token))
}

fn add_org_sync_social_routes(router: Router<AppState>) -> Router<AppState> {
    router
        .route(ROUTE_ORGS_MEMBERSHIPS, get(org_memberships))
        .route(ROUTE_ORGS_ACTIVE, post(set_active_org))
        .route(ROUTE_POLICY_AUTHORIZE, post(policy_authorize))
        .route(ROUTE_SYNC_TOKEN, post(sync_token))
        .route(ROUTE_SHOUTS, post(shouts_store))
        .route(ROUTE_WHISPERS, get(whispers_index).post(whispers_store))
        .route(ROUTE_WHISPERS_READ, patch(whispers_read))
}

fn add_payments_l402_routes(router: Router<AppState>) -> Router<AppState> {
    router
        .route(
            ROUTE_AGENT_PAYMENTS_WALLET,
            get(agent_payments_wallet).post(agent_payments_upsert_wallet),
        )
        .route(ROUTE_AGENT_PAYMENTS_BALANCE, get(agent_payments_balance))
        .route(
            ROUTE_AGENT_PAYMENTS_INVOICE,
            post(agent_payments_create_invoice),
        )
        .route(ROUTE_AGENT_PAYMENTS_PAY, post(agent_payments_pay_invoice))
        .route(
            ROUTE_AGENT_PAYMENTS_SEND_SPARK,
            post(agent_payments_send_spark),
        )
        .route(
            ROUTE_AGENTS_ME_WALLET,
            get(agent_payments_wallet).post(agent_payments_upsert_wallet),
        )
        .route(ROUTE_AGENTS_ME_BALANCE, get(agent_payments_balance))
        .route(ROUTE_PAYMENTS_INVOICE, post(agent_payments_create_invoice))
        .route(ROUTE_PAYMENTS_PAY, post(agent_payments_pay_invoice))
        .route(ROUTE_PAYMENTS_SEND_SPARK, post(agent_payments_send_spark))
        .route(ROUTE_L402_WALLET, get(l402_wallet))
        .route(ROUTE_L402_TRANSACTIONS, get(l402_transactions))
        .route(ROUTE_L402_TRANSACTION_BY_ID, get(l402_transaction_show))
        .route(ROUTE_L402_PAYWALLS, get(l402_paywalls))
        .route(ROUTE_L402_PAYWALLS, post(l402_paywall_create))
        .route(
            ROUTE_L402_PAYWALL_BY_ID,
            patch(l402_paywall_update).delete(l402_paywall_delete),
        )
        .route(ROUTE_L402_SETTLEMENTS, get(l402_settlements))
        .route(ROUTE_L402_DEPLOYMENTS, get(l402_deployments))
}

fn add_legacy_compat_routes(router: Router<AppState>) -> Router<AppState> {
    router
        .route(
            "/api/chat/guest-session",
            get(legacy_chat_guest_session_retired),
        )
        .route(ROUTE_LEGACY_CHAT_STREAM, post(legacy_chat_stream))
        .route(
            "/api/chats",
            get(legacy_chats_index).post(legacy_chats_store),
        )
        .route("/api/chats/:conversation_id", get(legacy_chats_show))
        .route(
            "/api/chats/:conversation_id/messages",
            get(legacy_chats_messages),
        )
        .route(ROUTE_LEGACY_CHATS_STREAM, post(legacy_chats_stream))
        .route("/api/chats/:conversation_id/runs", get(legacy_chats_runs))
        .route(
            "/api/chats/:conversation_id/runs/:run_id/events",
            get(legacy_chats_run_events),
        )
}

fn add_runtime_control_routes(
    router: Router<AppState>,
    thread_message_throttle_state: AppState,
    codex_control_request_throttle_state: AppState,
) -> Router<AppState> {
    router
        .route(ROUTE_RUNTIME_TOOLS_EXECUTE, post(runtime_tools_execute))
        .route(
            ROUTE_RUNTIME_SKILLS_TOOL_SPECS,
            get(runtime_skill_tool_specs_list).post(runtime_skill_tool_spec_store),
        )
        .route(
            ROUTE_RUNTIME_SKILLS_SKILL_SPECS,
            get(runtime_skill_specs_list).post(runtime_skill_spec_store),
        )
        .route(
            ROUTE_RUNTIME_SKILLS_SKILL_SPEC_PUBLISH,
            post(runtime_skill_spec_publish),
        )
        .route(
            ROUTE_RUNTIME_SKILLS_RELEASE,
            get(runtime_skill_release_show),
        )
        .route(ROUTE_RUNTIME_THREADS, get(list_runtime_threads))
        .route(
            ROUTE_RUNTIME_THREAD_MESSAGES,
            get(list_runtime_thread_messages)
                .post(send_thread_message)
                .route_layer(middleware::from_fn_with_state(
                    thread_message_throttle_state,
                    throttle_thread_message_gate,
                )),
        )
        .route(
            ROUTE_RUNTIME_CODEX_WORKERS,
            get(runtime_codex_workers_index).post(runtime_codex_workers_create),
        )
        .route(
            ROUTE_RUNTIME_CODEX_WORKER_BY_ID,
            get(runtime_codex_worker_show),
        )
        .route(
            ROUTE_RUNTIME_CODEX_WORKER_STREAM,
            get(runtime_codex_worker_stream),
        )
        .route(
            ROUTE_RUNTIME_CODEX_WORKER_EVENTS,
            post(runtime_codex_worker_events),
        )
        .route(
            ROUTE_RUNTIME_CODEX_WORKER_STOP,
            post(runtime_codex_worker_stop),
        )
        .route(
            ROUTE_RUNTIME_CODEX_WORKER_REQUESTS,
            post(runtime_codex_worker_request).route_layer(middleware::from_fn_with_state(
                codex_control_request_throttle_state,
                throttle_codex_control_request_gate,
            )),
        )
        .route(
            ROUTE_RUNTIME_WORKERS,
            get(runtime_workers_index).post(runtime_workers_create),
        )
        .route(ROUTE_RUNTIME_WORKER_BY_ID, get(runtime_worker_show))
        .route(
            ROUTE_RUNTIME_WORKER_HEARTBEAT,
            post(runtime_worker_heartbeat),
        )
        .route(ROUTE_RUNTIME_WORKER_STATUS, post(runtime_worker_transition))
}

fn add_admin_control_routes(router: Router<AppState>, admin_state: AppState) -> Router<AppState> {
    router
        .route(ROUTE_V1_CONTROL_STATUS, get(control_status))
        .route(ROUTE_V1_CONTROL_ROUTE_SPLIT_STATUS, get(route_split_status))
        .route(
            ROUTE_V1_CONTROL_ROUTE_SPLIT_OVERRIDE,
            post(route_split_override).route_layer(middleware::from_fn_with_state(
                admin_state.clone(),
                admin_email_gate,
            )),
        )
        .route(
            ROUTE_V1_CONTROL_ROUTE_SPLIT_EVALUATE,
            post(route_split_evaluate),
        )
        .route(
            ROUTE_V1_CONTROL_RUNTIME_ROUTING_STATUS,
            get(runtime_routing_status),
        )
        .route(
            ROUTE_V1_CONTROL_RUNTIME_ROUTING_EVALUATE,
            post(runtime_routing_evaluate),
        )
        .route(
            ROUTE_V1_CONTROL_RUNTIME_ROUTING_OVERRIDE,
            post(runtime_routing_override).route_layer(middleware::from_fn_with_state(
                admin_state,
                admin_email_gate,
            )),
        )
}
