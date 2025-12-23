//! Maud HTML templates

pub mod chat;
pub mod components;
pub mod context;
pub mod layout;
pub mod parallel;
pub mod permissions;

pub use layout::page;
pub use parallel::{parallel_agents_page, agents_list, AgentViewInfo, AgentViewStatus, IssueViewInfo, PlatformViewInfo};
pub use permissions::permissions_view;
