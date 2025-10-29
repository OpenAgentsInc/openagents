use clap::Parser;
use std::io::{self, Write};

#[derive(Parser, Debug)]
#[command(name = "fake-codex", version, about = "Minimal Codex CLI simulator for tests")]
struct Opts {
    #[arg(last = true)]
    rest: Vec<String>,
}

fn main() {
    let opts = Opts::parse();
    // Expect patterns like: exec --json [resume <id>] <prompt>
    if !opts.rest.iter().any(|s| s == "exec") || !opts.rest.iter().any(|s| s == "--json") {
        eprintln!("fake-codex: unsupported args: {:?}", opts.rest);
        std::process::exit(1);
    }
    let mut thread_id = "test-thread-123".to_string();
    for i in 0..opts.rest.len() {
        if opts.rest[i] == "resume" {
            if let Some(id) = opts.rest.get(i + 1) { thread_id = id.clone(); }
            break;
        }
    }
    let mut out = io::BufWriter::new(io::stdout());
    writeln!(out, "{}", serde_json::json!({
        "type": "thread.started",
        "thread_id": thread_id,
    })).unwrap();
    out.flush().ok();
    writeln!(out, "{}", serde_json::json!({
        "type": "item.completed",
        "item": {"id":"item_0","type":"reasoning","text":"**Thinking**"}
    })).unwrap();
    writeln!(out, "{}", serde_json::json!({
        "type": "item.completed",
        "item": {"id":"item_1","type":"agent_message","text":"Hello from fake codex"}
    })).unwrap();
    writeln!(out, "{}", serde_json::json!({"type":"turn.completed","usage":{"input_tokens":100,"output_tokens":5}})).unwrap();
    out.flush().ok();
}

