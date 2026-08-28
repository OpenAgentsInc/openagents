use std::net::SocketAddr;
use std::sync::Arc;

use clap::Parser;
use openagents_coder_api::{router, App, Config, Store};

#[derive(Parser, Debug)]
#[command(name = "openagents-coder-api")]
struct Args {
    /// Bind address, for example `127.0.0.1:4010`.
    #[arg(long)]
    bind: Option<String>,
    /// SQLite path. Defaults to `~/.openagents/coder-api/state.sqlite`.
    #[arg(long)]
    db: Option<std::path::PathBuf>,
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let args = Args::parse();
    let mut config = Config::from_env();
    if let Some(bind) = args.bind {
        config.public_origin = format!("http://{bind}");
        config.bind = bind;
    }
    if let Some(db) = args.db {
        config.db_path = db;
    }

    let store = Store::open(&config.db_path).unwrap_or_else(|error| {
        eprintln!(
            "openagents-coder-api: could not open {}: {error}",
            config.db_path.display()
        );
        std::process::exit(1);
    });
    let bind = config.bind.clone();
    let vercel = config.vercel_configured();
    let openrouter = config.openrouter_configured();
    tracing::info!(
        bind = %bind,
        db = %config.db_path.display(),
        vercel_configured = vercel,
        openrouter_configured = openrouter,
        "openagents-coder-api listening"
    );
    let app = App {
        config,
        store: Arc::new(store),
    };
    let listener = tokio::net::TcpListener::bind(&bind)
        .await
        .unwrap_or_else(|error| {
            eprintln!("openagents-coder-api: bind {bind} failed: {error}");
            std::process::exit(1);
        });
    axum::serve(listener, router(app)).await.expect("server");
}

#[allow(dead_code)]
fn _addr(bind: &str) -> Option<SocketAddr> {
    bind.parse().ok()
}
