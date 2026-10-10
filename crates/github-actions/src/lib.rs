//! Typed GitHub changes and the REST calls that make them, shared by the
//! web chat's GitHub tools (#11167, `crates/openagents-web/src/github_tools`)
//! and the CLI's `openagents issue|project` verbs (#11166).
//!
//! - [`action`]: an [`Action`] (open, comment on, or close an issue; set
//!   its status on a GitHub Projects board; open a pull request), read
//!   from a form's [`Fields`] with every value checked, and the confirm
//!   [`Card`] that says each change before it happens.
//! - [`argv`]: the same action as `openagents` command words, so a
//!   command the chat router proposed becomes a card, and a CLI verb
//!   becomes an action.
//! - [`rest`]: running an action, or one step of it, over GitHub's REST
//!   API through any [`rest::Api`] (`rest::Http` with a token, or a fake in
//!   tests). Boards use the REST `projectsV2` endpoints, as
//!   `scripts/dev/issue-board.sh` does, so they keep working when other
//!   tools have spent the GraphQL limit.
//!
//! - [`issues`]: the CLI's issue and board verbs (#11166) over a blocking
//!   [`issues::Rest`] transport: create, comment, close, reopen, list and
//!   view issues; add an issue to a board, set its status, and list a
//!   board's items. Moved here from `coder::github_rest`, so every GitHub
//!   REST call the product makes lives in this one crate.
//!
//! Nothing here reads a message's words to choose a change.

pub mod action;
pub mod argv;
pub mod issues;
pub mod rest;

#[cfg(test)]
mod tests;

pub use action::{Action, Board, Card, CloseReason, Fields, Tool, full_name};
pub use argv::{ArgvError, COMMANDS};
pub use rest::{Answer, Api, Done, Failure, Method, run};
