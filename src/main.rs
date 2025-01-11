mod event;
mod relay;
mod subscription;
mod db;
mod server;
mod configuration;

use actix_web::{web, App, HttpServer};
use actix_web_actors::ws;
use actix_cors::Cors;
use tokio::sync::broadcast;
use uuid::Uuid;
use std::sync::Arc;
use std::time::Duration;
use tracing::info;

use crate::event::Event;
use crate::relay::RelayWs;
use crate::db::Database;
use crate::configuration::get_configuration;

async fn root_route(
    req: actix_web::HttpRequest,
    stream: web::Payload,
    event_tx: web::Data<broadcast::Sender<Event>>,
    db: web::Data<Arc<Database>>,
) -> Result<actix_web::HttpResponse, actix_web::Error> {
    // Check if this is a WebSocket upgrade request
    if req.headers().contains_key("Upgrade") {
        let id = Uuid::new_v4().to_string();
        let ws = RelayWs::new(id, event_tx.get_ref().clone(), db.get_ref().clone());
        ws::start(ws, &req, stream)
    } else {
        // Not a WebSocket request, serve index.html
        Ok(actix_web::HttpResponse::Ok()
            .content_type("text/html")
            .body(std::fs::read_to_string("static/index.html").unwrap()))
    }
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    if std::env::var("RUST_LOG").is_err() {
        std::env::set_var("RUST_LOG", "info");
    }
    env_logger::init();
    dotenv::dotenv().ok();

    info!("Starting application...");
    info!("Environment: {}", std::env::var("APP_ENVIRONMENT").unwrap_or_else(|_| "not set".to_string()));
    info!("Port from env: {}", std::env::var("PORT").unwrap_or_else(|_| "not set".to_string()));
    
    // Log database-related environment variables
    info!("DATABASE_URL: {}", std::env::var("DATABASE_URL").is_ok());
    info!("PGHOST: {}", std::env::var("PGHOST").is_ok());
    info!("PGPORT: {}", std::env::var("PGPORT").is_ok());
    info!("PGUSER: {}", std::env::var("PGUSER").is_ok());
    info!("PGDATABASE: {}", std::env::var("PGDATABASE").is_ok());
    
    // Print current directory and its contents
    let current_dir = std::env::current_dir().expect("Failed to get current directory");
    info!("Current directory: {:?}", current_dir);
    if let Ok(entries) = std::fs::read_dir(&current_dir) {
        info!("Directory contents:");
        for entry in entries {
            if let Ok(entry) = entry {
                info!("  {:?}", entry.path());
            }
        }
    }

    // Load configuration
    info!("Loading configuration...");
    let configuration = get_configuration().expect("Failed to read configuration.");
    info!("Configuration loaded successfully");
    info!("App port: {}", configuration.application.port);
    info!("App host: {}", configuration.application.host);
    
    // Initialize database using configuration with retries
    info!("Initializing database connection...");
    let db = Arc::new(
        Database::new_with_options(
            configuration.database.connect_options(),
            configuration.database.max_connection_retries,
            Duration::from_secs(configuration.database.retry_interval_secs),
        )
        .await
        .expect("Failed to connect to database after all retries")
    );
    info!("Database connection established successfully");
    let db = web::Data::new(db);

    // Channel for broadcasting events to all connected clients
    let (event_tx, _): (broadcast::Sender<Event>, _) = broadcast::channel(1024);
    let event_tx = web::Data::new(event_tx);

    let address = format!(
        "{}:{}",
        configuration.application.host, configuration.application.port
    );
    info!("Starting server on {}", address);

    info!("Attempting to bind server...");
    let app_factory = move || {
        info!("Configuring new worker...");
        let cors = Cors::permissive();
        
        App::new()
            .wrap(cors)
            .app_data(event_tx.clone())
            .app_data(db.clone())
            .route("/", web::get().to(root_route))
            .configure(server::config::configure_app)
    };

    let mut server = HttpServer::new(app_factory)
    .bind(&address)
    .or_else(|e| {
        // Only attempt port increment in development/local environment
        if configuration.application.host == "127.0.0.1" {
            let mut port = configuration.application.port;
            while port < configuration.application.port + 10 {
                port += 1;
                let new_address = format!("{}:{}", configuration.application.host, port);
                info!("Address {} in use, trying {}", address, new_address);
                if let Ok(server) = HttpServer::new(app_factory.clone()).bind(&new_address) {
                    return Ok(server);
                }
            }
        }
        // If we're not in development or couldn't find a free port, return original error
        Err(e)
    })?;
    
    // Log the actual bound address
    let addresses = server.addrs();
    info!("Server addresses:");
    for addr in addresses {
        info!("🚀 Server ready at: http://{}", addr);
        info!("Admin endpoint: http://{}/admin/stats", addr);
    }
    
    info!("Starting server...");
    server.run().await
}
