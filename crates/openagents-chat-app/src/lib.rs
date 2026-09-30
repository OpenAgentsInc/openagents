//! Chat application state shared by desktop and phone adapters.
//! No wallet, SQLite, native widget, or platform host dependency belongs here.
pub use openagents_chat::{basic_chats, basic_coder, router};
pub mod chat_invites;
pub mod chats;
pub mod cli_run;
pub mod coder_list;
pub mod coder_tab;
pub mod conversation;
pub mod eval_cards;
pub mod first_run;
pub mod gym;
pub mod outbox;
pub mod transcripts;
pub mod wake;

#[cfg(test)]
mod gym_fixture {
    pub const REPORT: &str = include_str!("../../openagents-mobile/fixtures/gym-report.json");
}
