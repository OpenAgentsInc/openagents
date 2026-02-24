use web_time::Duration;

pub(crate) const AUTH_STORAGE_KEY: &str = "openagents.web.auth.v1";
pub(crate) const SYNC_STATE_STORAGE_KEY: &str = "openagents.web.sync.v1";
pub(crate) const KHALA_CHANNEL_TOPIC: &str = "sync:v1";
pub(crate) const KHALA_WS_VSN: &str = "2.0.0";
pub(crate) const KHALA_DEFAULT_TOPIC: &str = "runtime.codex_worker_events";
pub(crate) const KHALA_HEARTBEAT_INTERVAL: Duration = Duration::from_secs(20);
pub(crate) const KHALA_POLL_INTERVAL: Duration = Duration::from_secs(4);
pub(crate) const KHALA_RECONNECT_BASE_DELAY_MS: u64 = 750;
pub(crate) const KHALA_RECONNECT_MAX_DELAY_MS: u64 = 8_000;
pub(crate) const SYNC_PERSIST_MIN_INTERVAL_MS: u64 = 800;
pub(crate) const ROUTE_SCROLL_POSITION_CACHE_LIMIT: usize = 256;
pub(crate) const CODEX_CHAT_ROOT_ID: &str = "openagents-web-shell-chat";
pub(crate) const CODEX_CHAT_HEADER_ID: &str = "openagents-web-shell-chat-header";
pub(crate) const CODEX_CHAT_MESSAGES_ID: &str = "openagents-web-shell-chat-messages";
pub(crate) const CODEX_CHAT_QUICK_PROMPTS_ID: &str = "openagents-web-shell-chat-quick-prompts";
pub(crate) const CODEX_CHAT_COMPOSER_ID: &str = "openagents-web-shell-chat-composer";
pub(crate) const CODEX_CHAT_INPUT_ID: &str = "openagents-web-shell-chat-input";
pub(crate) const CODEX_CHAT_SEND_ID: &str = "openagents-web-shell-chat-send";
pub(crate) const CODEX_CHAT_PROMPT_0_ID: &str = "openagents-web-shell-chat-prompt-0";
pub(crate) const CODEX_CHAT_PROMPT_1_ID: &str = "openagents-web-shell-chat-prompt-1";
pub(crate) const CODEX_CHAT_PROMPT_2_ID: &str = "openagents-web-shell-chat-prompt-2";
pub(crate) const CODEX_CHAT_PROMPT_3_ID: &str = "openagents-web-shell-chat-prompt-3";
pub(crate) const AUTH_PANEL_ID: &str = "openagents-web-shell-auth-panel";
pub(crate) const AUTH_EMAIL_INPUT_ID: &str = "openagents-web-shell-auth-email";
pub(crate) const AUTH_CODE_INPUT_ID: &str = "openagents-web-shell-auth-code";
pub(crate) const AUTH_SEND_ID: &str = "openagents-web-shell-auth-send";
pub(crate) const AUTH_VERIFY_ID: &str = "openagents-web-shell-auth-verify";
pub(crate) const AUTH_RESTORE_ID: &str = "openagents-web-shell-auth-restore";
pub(crate) const AUTH_LOGOUT_ID: &str = "openagents-web-shell-auth-logout";
pub(crate) const SETTINGS_PANEL_ID: &str = "openagents-web-shell-settings-panel";
pub(crate) const SETTINGS_STATUS_ID: &str = "openagents-web-shell-settings-status";
pub(crate) const SETTINGS_PROFILE_NAME_ID: &str = "openagents-web-shell-settings-profile-name";
pub(crate) const SETTINGS_PROFILE_SAVE_ID: &str = "openagents-web-shell-settings-profile-save";
pub(crate) const SETTINGS_PROFILE_DELETE_ID: &str = "openagents-web-shell-settings-profile-delete";
pub(crate) const SETTINGS_AUTOPILOT_DISPLAY_NAME_ID: &str =
    "openagents-web-shell-settings-autopilot-display-name";
pub(crate) const SETTINGS_AUTOPILOT_TAGLINE_ID: &str =
    "openagents-web-shell-settings-autopilot-tagline";
pub(crate) const SETTINGS_AUTOPILOT_OWNER_ID: &str =
    "openagents-web-shell-settings-autopilot-owner";
pub(crate) const SETTINGS_AUTOPILOT_PERSONA_ID: &str =
    "openagents-web-shell-settings-autopilot-persona";
pub(crate) const SETTINGS_AUTOPILOT_VOICE_ID: &str =
    "openagents-web-shell-settings-autopilot-voice";
pub(crate) const SETTINGS_AUTOPILOT_PRINCIPLES_ID: &str =
    "openagents-web-shell-settings-autopilot-principles";
pub(crate) const SETTINGS_AUTOPILOT_SAVE_ID: &str = "openagents-web-shell-settings-autopilot-save";
pub(crate) const SETTINGS_RESEND_KEY_ID: &str = "openagents-web-shell-settings-resend-key";
pub(crate) const SETTINGS_RESEND_EMAIL_ID: &str = "openagents-web-shell-settings-resend-email";
pub(crate) const SETTINGS_RESEND_NAME_ID: &str = "openagents-web-shell-settings-resend-name";
pub(crate) const SETTINGS_RESEND_CONNECT_ID: &str = "openagents-web-shell-settings-resend-connect";
pub(crate) const SETTINGS_RESEND_DISCONNECT_ID: &str =
    "openagents-web-shell-settings-resend-disconnect";
pub(crate) const SETTINGS_RESEND_TEST_ID: &str = "openagents-web-shell-settings-resend-test";
pub(crate) const SETTINGS_GOOGLE_CONNECT_ID: &str = "openagents-web-shell-settings-google-connect";
pub(crate) const SETTINGS_GOOGLE_DISCONNECT_ID: &str =
    "openagents-web-shell-settings-google-disconnect";
pub(crate) const ADMIN_PANEL_ID: &str = "openagents-web-shell-admin-panel";
pub(crate) const ADMIN_STATUS_ID: &str = "openagents-web-shell-admin-status";
pub(crate) const ADMIN_WORKER_ID_ID: &str = "openagents-web-shell-admin-worker-id";
pub(crate) const ADMIN_WORKSPACE_ID: &str = "openagents-web-shell-admin-workspace";
pub(crate) const ADMIN_ADAPTER_ID: &str = "openagents-web-shell-admin-adapter";
pub(crate) const ADMIN_CREATE_ID: &str = "openagents-web-shell-admin-create";
pub(crate) const ADMIN_REFRESH_ID: &str = "openagents-web-shell-admin-refresh";
pub(crate) const ADMIN_STOP_REASON_ID: &str = "openagents-web-shell-admin-stop-reason";
pub(crate) const ADMIN_STOP_CONFIRM_ID: &str = "openagents-web-shell-admin-stop-confirm";
pub(crate) const ADMIN_STOP_ID: &str = "openagents-web-shell-admin-stop";
pub(crate) const ADMIN_REQUEST_METHOD_ID: &str = "openagents-web-shell-admin-request-method";
pub(crate) const ADMIN_REQUEST_PARAMS_ID: &str = "openagents-web-shell-admin-request-params";
pub(crate) const ADMIN_REQUEST_ID_ID: &str = "openagents-web-shell-admin-request-id";
pub(crate) const ADMIN_REQUEST_SEND_ID: &str = "openagents-web-shell-admin-request-send";
pub(crate) const ADMIN_EVENT_TYPE_ID: &str = "openagents-web-shell-admin-event-type";
pub(crate) const ADMIN_EVENT_PAYLOAD_ID: &str = "openagents-web-shell-admin-event-payload";
pub(crate) const ADMIN_EVENT_SEND_ID: &str = "openagents-web-shell-admin-event-send";
pub(crate) const ADMIN_STREAM_CURSOR_ID: &str = "openagents-web-shell-admin-stream-cursor";
pub(crate) const ADMIN_STREAM_TAIL_ID: &str = "openagents-web-shell-admin-stream-tail";
pub(crate) const ADMIN_STREAM_FETCH_ID: &str = "openagents-web-shell-admin-stream-fetch";
pub(crate) const DOM_READY_BUDGET_MS: u64 = 450;
pub(crate) const GPU_INIT_BUDGET_MS: u64 = 1_600;
pub(crate) const FIRST_FRAME_BUDGET_MS: u64 = 2_200;
pub(crate) const BOOT_TOTAL_BUDGET_MS: u64 = 2_500;
pub(crate) const CHAT_QUICK_PROMPTS: [&str; 4] = [
    "What tools do you have?",
    "Make a test OpenAgents API call",
    "What can you do with bitcoin?",
    "Explain what you can do with the OpenAgents API",
];
pub(crate) const CHAT_QUICK_PROMPT_IDS: [&str; 4] = [
    CODEX_CHAT_PROMPT_0_ID,
    CODEX_CHAT_PROMPT_1_ID,
    CODEX_CHAT_PROMPT_2_ID,
    CODEX_CHAT_PROMPT_3_ID,
];
pub(crate) const ADMIN_WORKER_ALLOWED_METHODS: [&str; 6] = [
    "thread/start",
    "thread/resume",
    "thread/list",
    "thread/read",
    "turn/start",
    "turn/interrupt",
];
pub(crate) const WEB_SHELL_COMPAT_CLIENT_BUILD_ID: &str = "20260221T130000Z";
pub(crate) const WEB_SHELL_COMPAT_PROTOCOL_VERSION: &str = "openagents.control.v1";
pub(crate) const WEB_SHELL_COMPAT_SCHEMA_VERSION: &str = "1";
