//! Root of the `codex-core` library.

// Prevent accidental direct writes to stdout/stderr in library code. All
// user-visible output must go through the appropriate abstraction (e.g.,
// the TUI or the tracing stack).
#![deny(clippy::print_stdout, clippy::print_stderr)]

pub mod api_bridge;
pub(crate) mod apply_patch;
pub mod auth;
pub mod bash;
pub(crate) mod client;
pub(crate) mod client_common;
pub mod codex;
pub(crate) mod codex_conversation;
pub(crate) mod compact_remote;
pub use codex_conversation::CodexConversation;
pub(crate) mod codex_delegate;
pub(crate) mod command_safety;
pub mod config;
pub mod config_loader;
pub(crate) mod context_manager;
pub mod custom_prompts;
pub mod env;
pub(crate) mod environment_context;
pub mod error;
pub mod exec;
pub mod exec_env;
pub(crate) mod exec_policy;
pub mod features;
pub(crate) mod flags;
pub mod git_info;
pub mod landlock;
pub mod mcp;
pub(crate) mod mcp_connection_manager;
pub mod openai_models;
pub use mcp_connection_manager::MCP_SANDBOX_STATE_CAPABILITY;
pub use mcp_connection_manager::MCP_SANDBOX_STATE_NOTIFICATION;
pub use mcp_connection_manager::SandboxState;
pub(crate) mod mcp_tool_call;
pub(crate) mod message_history;
pub(crate) mod model_provider_info;
pub mod parse_command;
pub mod powershell;
pub mod sandboxing;
pub(crate) mod stream_events_utils;
pub(crate) mod text_encoding;
pub mod token_data;
pub(crate) mod truncate;
pub(crate) mod unified_exec;
pub(crate) mod user_instructions;
pub use model_provider_info::CHAT_WIRE_API_DEPRECATION_SUMMARY;
pub use model_provider_info::DEFAULT_LMSTUDIO_PORT;
pub use model_provider_info::DEFAULT_OLLAMA_PORT;
pub use model_provider_info::LMSTUDIO_OSS_PROVIDER_ID;
pub use model_provider_info::ModelProviderInfo;
pub use model_provider_info::OLLAMA_OSS_PROVIDER_ID;
pub use model_provider_info::WireApi;
pub use model_provider_info::built_in_model_providers;
pub use model_provider_info::create_oss_provider_with_base_url;
pub(crate) mod conversation_manager;
pub(crate) mod event_mapping;
pub mod review_format;
pub mod review_prompts;
pub use crate::core::protocol::InitialHistory;
pub use conversation_manager::ConversationManager;
pub use conversation_manager::NewConversation;
// Re-export common auth types for workspace consumers
pub use auth::AuthManager;
pub use auth::CodexAuth;
pub mod default_client;
pub mod project_doc;
pub(crate) mod rollout;
pub(crate) mod safety;
pub mod seatbelt;
pub mod shell;
pub mod shell_snapshot;
pub mod skills;
pub mod spawn;
pub mod terminal;
pub(crate) mod tools;
pub mod turn_diff_tracker;
pub use rollout::ARCHIVED_SESSIONS_SUBDIR;
pub use rollout::INTERACTIVE_SESSION_SOURCES;
pub use rollout::RolloutRecorder;
pub use rollout::SESSIONS_SUBDIR;
pub use rollout::SessionMeta;
pub use rollout::find_conversation_path_by_id_str;
pub use rollout::list::ConversationItem;
pub use rollout::list::ConversationsPage;
pub use rollout::list::Cursor;
pub use rollout::list::parse_cursor;
pub use rollout::list::read_head_for_summary;
pub(crate) mod function_tool;
pub(crate) mod state;
pub(crate) mod tasks;
pub(crate) mod user_notification;
pub(crate) mod user_shell_command;
pub mod util;

pub use apply_patch::CODEX_APPLY_PATCH_ARG1;
pub use command_safety::is_dangerous_command;
pub use command_safety::is_safe_command;
pub use exec_policy::ExecPolicyError;
pub use exec_policy::load_exec_policy;
pub use safety::get_platform_sandbox;
pub use safety::set_windows_sandbox_enabled;
// Re-export the protocol types from the standalone `codex-protocol` crate so existing
// `codex_core::protocol::...` references continue to work across the workspace.
pub use crate::protocol::protocol;
// Re-export protocol config enums to ensure call sites can use the same types
// as those in the protocol crate when constructing protocol messages.
pub use crate::protocol::config_types as protocol_config_types;

pub use crate::protocol::models::ContentItem;
pub use crate::protocol::models::LocalShellAction;
pub use crate::protocol::models::LocalShellExecAction;
pub use crate::protocol::models::LocalShellStatus;
pub use crate::protocol::models::ResponseItem;
pub use client::ModelClient;
pub use client_common::Prompt;
pub use client_common::REVIEW_PROMPT;
pub use client_common::ResponseEvent;
pub use client_common::ResponseStream;
pub use compact::content_items_to_text;
pub use event_mapping::parse_turn_item;
pub mod compact;
pub mod otel_init;
