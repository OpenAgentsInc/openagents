//! An independent Rust runner. Install the built executable in the host's allowlist.
fn main() -> std::io::Result<()> {
    openagents_sdk::agents::runner::serve_once(
        std::io::stdin().lock(),
        std::io::stdout().lock(),
        |turn| Ok(format!("{} received: {}", turn.agent.name, turn.task)),
    )
}
