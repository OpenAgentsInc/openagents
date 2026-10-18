//! Run Claude Code inside an Archil sandbox: create a sandbox, run a command,
//! stream its output, then stop and delete the sandbox.
//!
//! ```text
//! export ARCHIL_API_KEY=... ARCHIL_REGION=aws-us-east-1 ANTHROPIC_API_KEY=...
//! cargo run --example claude_code -- "Write a hello world script in Python and run it"
//! ```
//!
//! The sandbox is stopped and deleted even if a step fails.

use std::io::Write;

use archil::{Archil, CreateSandboxRequest, ProcessStartOptions, ProcessStream, Sandbox};

const DEFAULT_PROMPT: &str =
    "Create hello.py that prints a greeting, run it, and show me the output.";

/// Run `command` in the sandbox, echoing stdout/stderr as the chunks arrive,
/// and return its exit code.
async fn run_streaming(
    sandbox: &Sandbox,
    command: &str,
    options: ProcessStartOptions,
) -> archil::Result<i64> {
    let mut process = sandbox
        .run(command, options.stream_output(true).collect_output(false))
        .await?;
    while let Some(chunk) = process.next_output().await {
        match chunk.stream {
            ProcessStream::Stdout => std::io::stdout().write_all(&chunk.data).ok(),
            ProcessStream::Stderr => std::io::stderr().write_all(&chunk.data).ok(),
        };
        std::io::stdout().flush().ok();
    }
    let result = process.wait().await?;
    Ok(result.exit_code.unwrap_or(-1))
}

async fn session(sandbox: &Sandbox, prompt: &str) -> archil::Result<i64> {
    // Claude Code is an npm package; the default image ships Node.js.
    println!("--- installing Claude Code");
    let code = run_streaming(
        sandbox,
        "npm install -g @anthropic-ai/claude-code",
        ProcessStartOptions::new().timeout_seconds(600),
    )
    .await?;
    if code != 0 {
        eprintln!("install failed with exit code {code}");
        return Ok(code);
    }

    println!("--- running claude");
    // The prompt travels in the environment so it never needs shell quoting.
    run_streaming(
        sandbox,
        r#"mkdir -p /workspace && cd /workspace && claude -p "$CLAUDE_PROMPT" --permission-mode acceptEdits"#,
        ProcessStartOptions::new().env("CLAUDE_PROMPT", prompt).timeout_seconds(900),
    )
    .await
}

#[tokio::main]
async fn main() -> archil::Result<()> {
    let prompt = std::env::args()
        .nth(1)
        .unwrap_or_else(|| DEFAULT_PROMPT.to_string());
    let anthropic_key = std::env::var("ANTHROPIC_API_KEY").map_err(|_| {
        archil::Error::Config("set ANTHROPIC_API_KEY so Claude Code can authenticate".into())
    })?;

    // Reads ARCHIL_API_KEY and ARCHIL_REGION.
    let client = Archil::from_env()?;

    println!("--- creating sandbox");
    let mut sandbox = client
        .sandboxes()
        .create(
            CreateSandboxRequest::new()
                .name(format!("claude-code-{}", std::process::id()))
                .vcpu_count(2)
                .mem_size_mib(4096)
                .env("ANTHROPIC_API_KEY", anthropic_key),
        )
        .await?;
    println!("sandbox {} is {}", sandbox.id, sandbox.status);

    let outcome = session(&sandbox, &prompt).await;

    println!("--- cleaning up");
    // A sandbox must be stopped before it can be deleted.
    if let Err(e) = sandbox.stop().await {
        eprintln!("stop failed: {e}");
    }
    if let Err(e) = sandbox.delete().await {
        eprintln!("delete failed: {e}");
    }

    let code = outcome?;
    println!("claude exited with code {code}");
    std::process::exit(if code == 0 { 0 } else { 1 });
}
