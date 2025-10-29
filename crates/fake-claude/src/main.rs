use clap::Parser;
use std::io::{self, Write};

#[derive(Parser, Debug)]
#[command(name = "fake-claude", version, about = "Minimal Claude Code CLI simulator for tests")]
struct Opts {
    #[arg(last = true)]
    rest: Vec<String>,
}

fn main() {
    let _opts = Opts::parse();
    // output a simple sequence of Claude-like JSON events
    let mut out = io::BufWriter::new(io::stdout());
    // thinking
    writeln!(out, "{}", serde_json::json!({
        "type": "content_block_delta",
        "delta": {"type":"thinking_delta","thinking":"Analyzing request..."}
    })).unwrap();
    // text delta
    writeln!(out, "{}", serde_json::json!({
        "type": "content_block_delta",
        "delta": {"type":"text_delta","text":"Hello from fake Claude"}
    })).unwrap();
    // tool use start
    writeln!(out, "{}", serde_json::json!({
        "type": "content_block_start",
        "content_block": {"type":"tool_use","id":"tu_1","name":"bash","input":{"command":"echo hi"}}
    })).unwrap();
    // tool result
    writeln!(out, "{}", serde_json::json!({
        "type": "tool_result",
        "tool_use_id": "tu_1",
        "is_error": false,
        "result": {"stdout":"hi\n"}
    })).unwrap();
    out.flush().ok();
}

