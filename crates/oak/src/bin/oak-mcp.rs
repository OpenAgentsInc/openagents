//! `oak-mcp` — the decision API caller as an MCP stdio server.
//!
//! The client launches this binary as a subprocess and speaks newline-
//! delimited JSON-RPC 2.0 on standard input and output: `initialize`,
//! `notifications/initialized`, then `tools/list` and `tools/call`. The
//! Inference tools are `list_models` and `classify`; documentation tools
//! read a bounded, versioned corpus without credentials. Credentials, the endpoint,
//! and the workspace come from operator configuration — the same
//! `OPENAGENTS_API_KEY`, `OPENAGENTS_BASE_URL`, `OPENAGENTS_WORKSPACE`,
//! and config file `oak` reads — never from tool arguments.
//!
//! ```text
//! oak-mcp [--url URL] [--config PATH] [--workspace ID]
//!         [--timeout SECS] [--retries N]
//! ```

use std::path::PathBuf;
use std::time::Duration;

use oak::mcp::{Options, PROTOCOL_VERSIONS, serve};

fn usage() -> ! {
    eprintln!(
        "usage: oak-mcp [--url URL] [--config PATH] [--workspace ID]\n       \
         [--timeout SECS] [--retries N]\n\n  \
         reads JSON-RPC messages on standard input and answers on standard\n  \
         output; supported protocol versions: {}",
        PROTOCOL_VERSIONS.join(", ")
    );
    std::process::exit(2);
}

fn main() {
    let mut options = Options {
        timeout: Duration::from_secs(60),
        retries: 3,
        ..Options::default()
    };
    let mut args = std::env::args().skip(1);
    while let Some(flag) = args.next() {
        match flag.as_str() {
            "--url" => options.url = args.next(),
            "--config" => options.config = args.next().map(PathBuf::from),
            "--workspace" => options.workspace = args.next(),
            "--timeout" => {
                options.timeout = args
                    .next()
                    .and_then(|value| value.parse::<u64>().ok())
                    .map(Duration::from_secs)
                    .unwrap_or_else(|| usage());
            }
            "--retries" => {
                options.retries = args
                    .next()
                    .and_then(|value| value.parse::<u32>().ok())
                    .unwrap_or_else(|| usage());
            }
            "version" | "--version" => {
                println!("oak-mcp {}", env!("CARGO_PKG_VERSION"));
                std::process::exit(0);
            }
            _ => usage(),
        }
    }
    let stdin = std::io::stdin();
    let stdout = std::io::stdout();
    let stderr = std::io::stderr();
    std::process::exit(serve(
        &options,
        std::io::BufReader::new(stdin.lock()),
        stdout.lock(),
        stderr.lock(),
    ));
}
