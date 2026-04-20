#![allow(clippy::all)]
#![allow(
    clippy::print_stdout,
    reason = "This binary intentionally writes the final response or JSONL events to stdout."
)]
#![allow(
    clippy::print_stderr,
    reason = "This binary may surface non-interactive failures on stderr."
)]

#[path = "../codex_exec.rs"]
mod codex_exec;

fn main() -> anyhow::Result<()> {
    codex_exec::main_entry()
}
