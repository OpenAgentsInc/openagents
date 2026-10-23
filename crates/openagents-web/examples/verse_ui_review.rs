//! Loopback-only review server using the real Verse routes and disposable state.
use std::{
    net::{Ipv4Addr, SocketAddr},
    path::PathBuf,
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let state = PathBuf::from(
        std::env::args()
            .nth(1)
            .ok_or("Pass a new scratch directory")?,
    );
    std::fs::create_dir(&state)?;
    let listener = tokio::net::TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await?;
    println!("http://{}", listener.local_addr()?);
    let mut config = openagents_web::Config::development(state.join("tasks"));
    config.port = listener.local_addr()?.port();
    axum::serve(
        listener,
        openagents_web::router(config).into_make_service_with_connect_info::<SocketAddr>(),
    )
    .with_graceful_shutdown(async {
        let _ = tokio::signal::ctrl_c().await;
    })
    .await?;
    Ok(())
}
