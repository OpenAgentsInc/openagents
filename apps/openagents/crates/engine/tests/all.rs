//! Every integration test of this package in one binary (#12199): each
//! file beside this one is a module here, so a change links the package
//! once instead of once per file. Tests that change process-wide state
//! (environment variables) stay binaries of their own; see Cargo.toml.

mod adapter_chaos_live;
mod auth;
mod born_chat2_race;
mod chat_forks;
mod codex_catalog;
mod codex_login_resolution;
mod codex_subagents;
mod cursor_live;
mod device_routing;
mod diff_sync_churn;
mod e2e;
mod keepers;
mod local_first;
mod local_import;
mod local_profiles;
mod message_queue;
mod openagents_account;
mod peer_direct;
mod pi_resume;
mod previews;
mod project_action_commands;
mod projectless;
mod queue_order_live;
mod queued_attachments;
mod relay_delivery;
mod restart_resume;
mod rich_composer_delivery;
mod runtime_survival;
mod session_publication;
mod steer_now_live;
mod steer_tool_live;
mod steering_live;
mod sync_resources;
mod terminal_keepers;
mod transcript_salvage;
mod unarchive_on_send;
mod voice_runtime;
mod workspace_files;
mod workspace_sync;
mod worktree_on_run;
