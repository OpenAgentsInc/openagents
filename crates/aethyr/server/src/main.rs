use af_mmorpg_core::WorldConfig;
use af_world_server::{AppState, ServerSurface, default_origins, router, run_world};
use std::{net::SocketAddr, path::PathBuf};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut listen: SocketAddr = "127.0.0.1:55319".parse()?;
    let mut world_path = None::<PathBuf>;
    let mut origins = Vec::new();
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--listen" => listen = args.next().ok_or("Missing listen address.")?.parse()?,
            "--world" => world_path = Some(args.next().ok_or("Missing world file.")?.into()),
            "--allow-origin" => origins.push(args.next().ok_or("Missing browser origin.")?),
            "--help" => {
                println!(
                    "af-world-server [--listen 127.0.0.1:55319] [--world FILE] [--allow-origin ORIGIN]"
                );
                return Ok(());
            }
            _ => return Err(format!("Unknown option: {arg}").into()),
        }
    }
    let value = if let Some(path) = world_path {
        let value: serde_json::Value = serde_json::from_slice(&std::fs::read(path)?)?;
        value.get("simulation").cloned().unwrap_or(value)
    } else {
        serde_json::to_value(WorldConfig::default())?
    };
    let config = serde_json::from_value(value.clone())?;
    let colliders = if let Some(colliders) = value.get("colliders") {
        serde_json::from_value(colliders.clone())?
    } else {
        Vec::new()
    };
    let surface = ServerSurface {
        height_fn: |x, z| af_mmorpg_core::terrain::height(x as f64, z as f64) as f32,
        collision: af_kernel::CollisionScene::new(0., colliders)?,
        procedural_trees: true,
    };
    let state = AppState::new(
        config,
        surface,
        if origins.is_empty() {
            default_origins()
        } else {
            origins
        },
    )?;
    let listener = tokio::net::TcpListener::bind(listen).await?;
    println!(
        "World server listening at http://{}",
        listener.local_addr()?
    );
    let (stop, receiver) = tokio::sync::watch::channel(false);
    let simulation = tokio::spawn(run_world(state.clone(), receiver));
    let shutdown = state.shutting_down.clone();
    axum::serve(listener, router(state))
        .with_graceful_shutdown(async move {
            let _ = tokio::signal::ctrl_c().await;
            shutdown.store(true, std::sync::atomic::Ordering::Relaxed);
        })
        .await?;
    let _ = stop.send(true);
    let _ = simulation.await;
    Ok(())
}
