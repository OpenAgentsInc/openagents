//! Every integration test of this package in one binary (#12199): each
//! file beside this one is a module here, so a change links the package
//! once instead of once per file. Tests that change process-wide state
//! (environment variables) stay binaries of their own; see Cargo.toml.

#[path = "../../discovery/tests/support/curated.rs"]
mod curated;
#[path = "../../coder-control/src/tests/relay.rs"]
mod relay;
#[path = "support/sales_binary.rs"]
mod sales_binary;

mod accounts_usage;
mod agent_crew;
mod brainstorm_guidance;
mod browser;
mod chamber_reach;
mod chat;
mod cloud_engine;
mod coder;
mod completions;
mod customer;
mod discover;
mod eval;
mod ext_eval;
mod foreman_report;
mod friendly_errors;
mod help;
mod host_first_run;
mod kb_defaults;
mod keys;
mod labs;
mod lease;
mod lease_place;
mod mcp_serve;
mod plugin_background;
mod plugin_discovery;
mod plugin_team;
mod projects_sharing;
mod projects_uploads;
mod provider_key;
mod sales;
mod sales_agents;
mod sales_earned;
mod sales_email;
mod sales_meetings;
mod sales_models;
mod sales_partners;
mod sales_paul;
mod sales_privacy;
mod sales_replies;
mod sales_service;
mod scratch;
mod session;
mod studio_host;
mod task_list;
mod terminal;
mod token_env;
mod verify_card;
mod verse_args;
mod workflow_template;
