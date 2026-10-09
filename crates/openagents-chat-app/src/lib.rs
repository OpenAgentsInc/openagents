//! Chat application state shared by desktop and phone adapters.
//! No wallet, SQLite, native widget, or platform host dependency belongs here.
pub use openagents_chat::{basic_chats, basic_coder, router};
pub mod attention;
pub mod chat_invites;
pub mod chats;
pub mod cli_run;
pub mod coder_list;
pub mod coder_tab;
pub mod computer_chats;
pub mod conversation;
pub mod cues;
pub mod engine;
pub mod eval_cards;
pub mod feedback;
pub mod first_run;
pub mod gym;
pub mod host_threads;
pub mod hosted;
pub mod landing;
pub mod outbox;
pub mod preferences;
pub mod route_map;
pub mod transcripts;
pub mod wake;
pub mod watchers;

#[cfg(test)]
mod copy_guard_tests;

#[cfg(test)]
mod gym_fixture {
    pub const REPORT: &str = include_str!("../../openagents-mobile/fixtures/gym-report.json");
}
pub mod cards;
pub mod changes;
pub mod projection;
pub mod session;
pub mod subagents;

pub mod attachments;

pub mod chat_list;

pub mod command_panel;
pub mod commands;

pub mod task_chat;

pub mod coder_run;

pub mod decision;
pub mod plan_panel;

pub mod retained;

pub mod review_comments;

pub mod visual;
