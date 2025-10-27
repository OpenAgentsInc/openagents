use clap::Parser;
use bore_cli::client::Client;
use tracing::{info, error};
use tracing_subscriber::prelude::*;

#[derive(Parser, Debug, Clone)]
#[command(name = "oa-tunnel", about = "OpenAgents tunnel launcher (bore client)", version)]
struct Opts {
    /// Local host to forward
    #[arg(long, default_value = "127.0.0.1")]
    local_host: String,

    /// Local port to forward
    #[arg(long, default_value_t = 8787)]
    local_port: u16,

    /// Remote bore server (hostname)
    #[arg(long, default_value = "bore.pub")]
    to: String,

    /// Desired remote port (0 lets server pick)
    #[arg(long, default_value_t = 0)]
    port: u16,

    /// Optional shared secret for authentication
    #[arg(long)]
    secret: Option<String>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    init_tracing();
    let opts = Opts::parse();

    let client = Client::new(
        &opts.local_host,
        opts.local_port,
        &opts.to,
        opts.port,
        opts.secret.as_deref(),
    )
    .await?;

    let remote_port = client.remote_port();
    println!("ws://{}:{}/ws", opts.to, remote_port);
    info!(remote_port, "listening via bore");

    if let Err(e) = client.listen().await {
        error!(?e, "bore client exited with error");
    }
    Ok(())
}

fn init_tracing() {
    use tracing_subscriber::{EnvFilter, fmt};
    let _ = tracing_subscriber::registry()
        .with(EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()))
        .with(fmt::layer())
        .try_init();
}
