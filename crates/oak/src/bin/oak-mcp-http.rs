//! `oak-mcp-http` — the decision API caller as an MCP Streamable HTTP
//! server.
//!
//! The client speaks JSON-RPC 2.0 over `POST /mcp`: `initialize`
//! mints a session and returns it in `Mcp-Session-Id`, requests answer
//! 200 with the response document, notifications answer 202, and
//! `DELETE /mcp` ends the session. The tool set is the one `oak-mcp`
//! serves over stdio — the same dispatch, the same schemas.
//!
//! A caller may send `Authorization: Bearer <key>`; the key is
//! forwarded to the service for that call and never stored. With no
//! header the tools resolve the operator configuration — the same
//! `OPENAGENTS_API_KEY`, `OPENAGENTS_BASE_URL`, `OPENAGENTS_WORKSPACE`,
//! and config file `oak` reads — never a tool argument or a flag.
//!
//! ```text
//! oak-mcp-http [--listen ADDR] [--url URL] [--config PATH]
//!              [--workspace ID] [--timeout SECS] [--retries N]
//!              [--allow-origin ORIGIN]...
//! ```

use std::net::SocketAddr;
use std::path::PathBuf;
use std::time::Duration;

use oak::mcp::{Options, PROTOCOL_VERSIONS};
use oak::mcp_http::{HttpOptions, serve};

fn usage() -> ! {
    eprintln!(
        "usage: oak-mcp-http [--listen ADDR] [--url URL] [--config PATH]\n       \
         [--workspace ID] [--timeout SECS] [--retries N]\n       \
         [--allow-origin ORIGIN]...\n\n  \
         serves POST /mcp — JSON-RPC over Streamable HTTP; supported\n  \
         protocol versions: {}",
        PROTOCOL_VERSIONS.join(", ")
    );
    std::process::exit(2);
}

#[tokio::main]
async fn main() {
    let mut options = HttpOptions {
        options: Options {
            timeout: Duration::from_secs(60),
            retries: 3,
            ..Options::default()
        },
        origins: Vec::new(),
    };
    let mut listen = SocketAddr::from(([127, 0, 0, 1], 8765));
    let mut args = std::env::args().skip(1);
    while let Some(flag) = args.next() {
        match flag.as_str() {
            "--listen" => {
                listen = args
                    .next()
                    .and_then(|value| value.parse::<SocketAddr>().ok())
                    .unwrap_or_else(|| usage());
            }
            "--url" => options.options.url = args.next(),
            "--config" => options.options.config = args.next().map(PathBuf::from),
            "--workspace" => options.options.workspace = args.next(),
            "--timeout" => {
                options.options.timeout = args
                    .next()
                    .and_then(|value| value.parse::<u64>().ok())
                    .map(Duration::from_secs)
                    .unwrap_or_else(|| usage());
            }
            "--retries" => {
                options.options.retries = args
                    .next()
                    .and_then(|value| value.parse::<u32>().ok())
                    .unwrap_or_else(|| usage());
            }
            "--allow-origin" => {
                options.origins.push(args.next().unwrap_or_else(|| usage()));
            }
            "version" | "--version" => {
                println!("oak-mcp-http {}", env!("CARGO_PKG_VERSION"));
                std::process::exit(0);
            }
            _ => usage(),
        }
    }
    let listener = match tokio::net::TcpListener::bind(listen).await {
        Ok(listener) => listener,
        Err(error) => {
            eprintln!("oak-mcp-http: cannot listen on {listen}: {error}");
            std::process::exit(1);
        }
    };
    eprintln!("oak-mcp-http: serving http://{listen}/mcp");
    if let Err(error) = serve(options, listener).await {
        eprintln!("oak-mcp-http: {error}");
        std::process::exit(1);
    }
}
