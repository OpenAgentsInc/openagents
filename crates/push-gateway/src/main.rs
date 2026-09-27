//! `push-gateway`: hold APNs and FCM credentials for a relay's PL executor.
//!
//! Configuration comes from `PUSH_GATEWAY_*` variables; credentials come only
//! from the files they name. See `docs/deployment/push-gateway.md`.

use std::process::ExitCode;

#[tokio::main]
async fn main() -> ExitCode {
    let config = match push_gateway::server::Config::from_env() {
        Ok(config) => config,
        Err(message) => {
            eprintln!("push-gateway: {message}");
            return ExitCode::FAILURE;
        }
    };
    let running = match push_gateway::server::start(config).await {
        Ok(running) => running,
        Err(message) => {
            eprintln!("push-gateway: {message}");
            return ExitCode::FAILURE;
        }
    };
    eprintln!(
        "push-gateway: delivery on {}, registration on {}",
        running.delivery_addr, running.registration_addr
    );
    wait_for_signal().await;
    eprintln!("push-gateway: stopping");
    running.stop().await;
    ExitCode::SUCCESS
}

async fn wait_for_signal() {
    #[cfg(unix)]
    {
        use tokio::signal::unix::{SignalKind, signal};
        if let Ok(mut terminate) = signal(SignalKind::terminate()) {
            tokio::select! {
                _ = terminate.recv() => {}
                _ = tokio::signal::ctrl_c() => {}
            }
            return;
        }
    }
    let _ = tokio::signal::ctrl_c().await;
}
