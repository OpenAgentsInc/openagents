//! Transport layer for communicating with the Codex CLI.

mod process;

pub use process::{ProcessTransport, find_codex_executable};
