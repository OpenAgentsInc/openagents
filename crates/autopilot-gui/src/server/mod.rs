//! Local web server for autopilot GUI

mod routes;
mod state;
pub mod ws;

pub use state::AppState;

use actix_web::{middleware, web, App, HttpServer};
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::info;

/// Autopilot GUI web server
pub struct Server {
    port: u16,
}

impl Server {
    /// Create a new server instance
    pub fn new(port: u16) -> Self {
        Self { port }
    }

    /// Start the server
    pub async fn start(self) -> anyhow::Result<()> {
        let state = Arc::new(RwLock::new(AppState::default()));

        info!("Starting autopilot GUI server on http://localhost:{}", self.port);

        HttpServer::new(move || {
            App::new()
                .app_data(web::Data::new(state.clone()))
                .wrap(middleware::Logger::default())
                .configure(routes::configure)
        })
        .bind(("127.0.0.1", self.port))?
        .run()
        .await?;

        Ok(())
    }
}

impl Default for Server {
    fn default() -> Self {
        Self::new(3847)
    }
}
