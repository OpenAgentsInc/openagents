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
use tracing::{info, error};

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

    error!("Starting application...");
    error!("Environment: {}", std::env::var("APP_ENVIRONMENT").unwrap_or_else(|_| "not set".to_string()));
    error!("Port from env: {}", std::env::var("PORT").unwrap_or_else(|_| "not set".to_string()));
    
    // Log database-related environment variables
    error!("DATABASE_URL present: {}", std::env::var("DATABASE_URL").is_ok());
    error!("APP_DATABASE__HOST present: {}", std::env::var("APP_DATABASE__HOST").is_ok());
    error!("APP_DATABASE__PORT present: {}", std::env::var("APP_DATABASE__PORT").is_ok());
    error!("APP_DATABASE__USERNAME present: {}", std::env::var("APP_DATABASE__USERNAME").is_ok());
    error!("APP_DATABASE__DATABASE_NAME present: {}", std::env::var("APP_DATABASE__DATABASE_NAME").is_ok());
    
    // Print current directory and its contents
    let current_dir = std::env::current_dir().expect("Failed to get current directory");
    error!("Current directory: {:?}", current_dir);
    if let Ok(entries) = std::fs::read_dir(&current_dir) {
        error!("Directory contents:");
        for entry in entries {
            if let Ok(entry) = entry {
                error!("  {:?}", entry.path());
            }
        }
    }

    // Load configuration
    error!("Loading configuration...");
    let configuration = get_configuration().expect("Failed to read configuration.");
    error!("Configuration loaded successfully");
    error!("App port: {}", configuration.application.port);
    error!("App host: {}", configuration.application.host);
    
    // Initialize database
    error!("Initializing database connection...");
    let db = Arc::new(
        Database::new_with_options(configuration.database.connect_options())
            .await
            .expect("Failed to connect to database")
    );
    error!("Database connection established successfully");
    let db = web::Data::new(db);

    // Channel for broadcasting events to all connected clients
    let (event_tx, _): (broadcast::Sender<Event>, _) = broadcast::channel(1024);
    let event_tx = web::Data::new(event_tx);

    let address = format!(
        "{}:{}",
        configuration.application.host, configuration.application.port
    );
    error!("Starting server on {}", address);

    error!("Attempting to bind server...");
    let app_factory = move || {
        error!("Configuring new worker...");
        let cors = Cors::permissive();
        
        App::new()
            .wrap(cors)
            .app_data(event_tx.clone())
            .app_data(db.clone())
            .route("/", web::get().to(root_route))
            .configure(server::config::configure_app)
    };

    let factory = app_factory.clone();
    let server = HttpServer::new(factory)
    .bind(&address)
    .or_else(|e| {
        // Only attempt port increment in development/local environment
        if configuration.application.host == "127.0.0.1" {
            let mut port = configuration.application.port;
            while port < configuration.application.port + 10 {
                port += 1;
                let new_address = format!("{}:{}", configuration.application.host, port);
                error!("Address {} in use, trying {}", address, new_address);
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
    error!("Server addresses:");
    for addr in addresses {
        error!("🚀 Server ready at: http://{}", addr);
        error!("Admin endpoint: http://{}/admin/stats", addr);
    }
    
    error!("Starting server...");
    server.run().await
}