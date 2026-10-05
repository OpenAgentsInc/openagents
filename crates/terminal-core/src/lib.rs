//! Application state shared by terminal mounts. Shell output grants no authority.

pub mod blocks;
pub mod bridge;
pub mod context;
pub mod proposals;
pub mod route;
pub mod zsh;

pub mod application;
pub mod ascii;
pub mod control;
pub mod copy;
pub mod input;
pub mod keys;
pub mod layout;
pub mod mouse;
pub mod paper;
pub mod pty;
pub mod select;
pub mod smart;
pub mod stats;
pub use application::Application as Overlay;
pub(crate) use application::scroll;
pub use application::{Application, HELP, PANE_BYTES, UPDATE_BUDGET};
pub use input::KeyIn;

#[cfg(test)]
mod application_tests;
