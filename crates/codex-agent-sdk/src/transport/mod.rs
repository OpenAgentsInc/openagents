//! Transport layer for communicating with the Codex CLI.

mod process;

pub use process::{find_codex_executable, ProcessTransport};
