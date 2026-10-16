use coder_sync::server::{StateData, config::Config, db::Database, rooms::Rooms, storage::Bucket};

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .json()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "coder_sync=info".into()),
        )
        .init();
    if let Err(message) = run().await {
        tracing::error!(message, "Server stopped");
        std::process::exit(1);
    }
}

async fn run() -> Result<(), &'static str> {
    let config = Config::from_env()?;
    let database =
        Database::new(&config.database_url).map_err(|_| "Invalid database configuration.")?;
    database
        .migrate()
        .await
        .map_err(|_| "Could not prepare the database.")?;
    let bucket = Bucket::google(config.bucket).map_err(|_| "Invalid bucket configuration.")?;
    bucket
        .probe()
        .await
        .map_err(|_| "Could not read and write the bucket.")?;
    let state = StateData {
        database,
        bucket,
        rooms: Rooms::default(),
        devices: Default::default(),
        auth: config.auth,
        revision: config.revision,
    };
    let listener = tokio::net::TcpListener::bind(config.listen)
        .await
        .map_err(|_| "Could not listen for connections.")?;
    tracing::info!(listen=%config.listen, "Server ready");
    axum::serve(listener, coder_sync::server::router(state))
        .with_graceful_shutdown(async {
            let _ = tokio::signal::ctrl_c().await;
        })
        .await
        .map_err(|_| "Could not serve requests.")
}
