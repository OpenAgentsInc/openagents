//! nostr-relay — a hardened Nostr relay. One Rust binary, one Postgres.
//!
//!   nostr::domain — NIP-01 primitives: event, tags, filters, canonical ID,
//!              replacement addresses, deletion semantics (owned, no
//!              third-party Nostr crate)
//!   store/   — Postgres: admission transaction, ingest_seq, indexes,
//!              LISTEN/NOTIFY fanout
//!   gateway/ — HTTP/WebSocket protocol server: authentication, management,
//!              COUNT/search, Blossom media, subscription index, EOSE handoff,
//!              ephemeral lane

mod operations;

use nostr_relay::gateway::{Gateway, GatewayConfig, GatewayError};

#[tokio::main]
async fn main() {
    if let Err(error) = run().await {
        let message = serde_json::json!({
            "level": "error",
            "message": error.to_string(),
        });
        eprintln!("{message}");
        std::process::exit(1);
    }
}

async fn run() -> Result<(), GatewayError> {
    let mut arguments = std::env::args().skip(1);
    if let Some(command) = arguments.next() {
        if arguments.next().is_some() {
            return Err(GatewayError::Config(
                "operator command accepts no positional arguments".to_owned(),
            ));
        }
        return match command.as_str() {
            "import-jsonl" => operations::import_signed_events().await,
            _ => Err(GatewayError::Config(format!(
                "unknown operator command: {command}"
            ))),
        };
    }
    let config = GatewayConfig::from_env()?;
    let gateway = Gateway::start(config).await?;
    let address = gateway.local_addr();
    let shutdown = gateway.shutdown_handle();
    let mut server = tokio::spawn(gateway.run());
    println!(
        "{}",
        serde_json::json!({
            "level": "info",
            "message": "nostr-relay listening",
            "address": address.to_string(),
            "version": env!("CARGO_PKG_VERSION"),
        })
    );
    tokio::select! {
        result = &mut server => join_server(result),
        signal = shutdown_signal() => {
            signal?;
            shutdown.shutdown();
            join_server(server.await)
        }
    }
}

fn join_server(
    result: Result<Result<(), GatewayError>, tokio::task::JoinError>,
) -> Result<(), GatewayError> {
    result.map_err(|error| GatewayError::Internal(format!("server task failed: {error}")))?
}

async fn shutdown_signal() -> Result<(), GatewayError> {
    #[cfg(unix)]
    {
        let mut terminate =
            tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
                .map_err(GatewayError::Io)?;
        tokio::select! {
            result = tokio::signal::ctrl_c() => result.map_err(GatewayError::Io),
            _ = terminate.recv() => Ok(()),
        }
    }
    #[cfg(not(unix))]
    {
        tokio::signal::ctrl_c().await.map_err(GatewayError::Io)
    }
}
