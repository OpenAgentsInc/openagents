//! `gateway` — the keyed HTTP front of the Decision API's serving half.
//!
//! One command, one config file:
//!
//! ```text
//! gateway --config gateway.json
//! ```
//!
//! The config names the listen address, the registry directory
//! (`registry.json`, `keys.json`, `quota-ledger.jsonl`, and
//! `receipts.jsonl` all live there), and the endpoint standing behind
//! each door. The process binds the public address and serves
//! `POST /v1/systemone`, `GET /v1/models`, and `GET /healthz` until it
//! is stopped.

use std::net::SocketAddr;
use std::path::PathBuf;
use std::process::ExitCode;

use gateway::config::Config;
use gateway::serve;

/// The command's arguments: just the config file.
struct Options {
    /// The `gateway.json` to load.
    config: PathBuf,
}

/// Parse the arguments — one required flag, nothing else to get wrong.
fn options(args: &[String]) -> Result<Options, String> {
    let mut config = None;
    let mut rest = args.iter().skip(1);
    while let Some(arg) = rest.next() {
        match arg.as_str() {
            "--config" => {
                config = Some(
                    rest.next()
                        .ok_or_else(|| "--config names a file".to_string())?
                        .into(),
                );
            }
            other => return Err(format!("unknown argument `{other}`")),
        }
    }
    Ok(Options {
        config: config.ok_or_else(|| "the gateway needs `--config gateway.json`".to_string())?,
    })
}

#[tokio::main]
async fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().collect();
    let options = match options(&args) {
        Ok(options) => options,
        Err(message) => {
            eprintln!("{message}");
            return ExitCode::FAILURE;
        }
    };
    let config = match Config::load(&options.config) {
        Ok(config) => config,
        Err(message) => {
            eprintln!("{message}");
            return ExitCode::FAILURE;
        }
    };
    let state = match serve::ServeState::open(config.clone()) {
        Ok(state) => state,
        Err(trouble) => {
            eprintln!("{trouble}");
            return ExitCode::FAILURE;
        }
    };
    let address: SocketAddr = match config.listen.parse() {
        Ok(address) => address,
        Err(error) => {
            eprintln!("{}: `{error}`", config.listen);
            return ExitCode::FAILURE;
        }
    };
    let listener = match tokio::net::TcpListener::bind(address).await {
        Ok(listener) => listener,
        Err(error) => {
            eprintln!("{address}: {error}");
            return ExitCode::FAILURE;
        }
    };
    eprintln!("gateway listening on {address}");
    if let Err(error) = axum::serve(listener, serve::router(state)).await {
        eprintln!("the listener stopped: {error}");
        return ExitCode::FAILURE;
    }
    ExitCode::SUCCESS
}
