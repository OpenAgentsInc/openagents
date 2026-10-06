//! Application state shared by terminal mounts. Shell output grants no authority.

pub mod bash;
pub mod blocks;
pub mod bridge;
pub mod context;
pub mod correct;
pub mod fish;
pub mod integration;
pub mod proposals;
pub mod route;
pub mod rules;
pub mod search;
pub mod zsh;

pub mod application;
pub mod ascii;
pub mod control;
pub mod copy;
pub mod excerpt;
pub mod files;
pub mod gym;
pub mod input;
pub mod keys;
pub mod knowledge;
pub mod layout;
pub mod mouse;
pub mod opening;
pub mod paper;
pub mod paste;
pub mod pty;
pub mod resources;
pub mod run;
pub mod select;
pub mod smart;
pub mod stats;
pub mod thread;
pub use application::Application as Overlay;
pub(crate) use application::scroll;
pub use application::{Application, HELP, PANE_BYTES, UPDATE_BUDGET};
pub use input::KeyIn;

#[cfg(test)]
mod application_tests;
