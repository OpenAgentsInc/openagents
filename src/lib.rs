pub mod configuration;
pub mod database;
mod emailoptin;
pub mod nostr;
pub mod server;

use actix_cors::Cors;
use actix_web::{web, App, HttpServer};
use actix_web_actors::ws;
use std::sync::Arc;
use tokio::sync::broadcast;
use tracing::{debug, info, warn};
use uuid::Uuid;

use crate::configuration::get_configuration;
use crate::nostr::db::Database;
use crate::nostr::event::Event;
use crate::nostr::relay::RelayWs;

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
pub async fn run() -> std::io::Result<()> {
    if std::env::var("RUST_LOG").is_err() {
        std::env::set_var("RUST_LOG", "info");
    }
    env_logger::init();
    dotenv::dotenv().ok();

    info!("🚀 Starting OpenAgents...");

    // Load configuration
    debug!("Loading configuration...");
    let configuration = get_configuration().expect("Failed to read configuration.");

    // Initialize database
    info!("Connecting to database...");
    let db = Arc::new(
        Database::new_with_options(configuration.database.connect_options())
            .await
            .expect("Failed to connect to database"),
    );
    info!("✅ Database connected");
    let db = web::Data::new(db);

    // Channel for broadcasting events to all connected clients
    let (event_tx, _): (broadcast::Sender<Event>, _) = broadcast::channel(1024);
    let event_tx = web::Data::new(event_tx);

    let address = format!(
        "{}:{}",
        configuration.application.host, configuration.application.port
    );
    info!("Starting server on {}", address);

    let app_factory = move || {
        debug!("Initializing worker...");
        let cors = Cors::permissive();

        App::new()
            .wrap(cors)
            .app_data(event_tx.clone())
            .app_data(db.clone())
            .route("/", web::get().to(root_route))
            .configure(server::config::configure_app)
    };

    let factory = app_factory.clone();
    let server = HttpServer::new(factory).bind(&address).or_else(|e| {
        // Only attempt port increment in development/local environment
        if configuration.application.host == "127.0.0.1" {
            let mut port = configuration.application.port;
            while port < configuration.application.port + 10 {
                port += 1;
                let new_address = format!("{}:{}", configuration.application.host, port);
                warn!(
                    "Port {} in use, trying {}",
                    configuration.application.port, port
                );
                if let Ok(server) = HttpServer::new(app_factory.clone()).bind(&new_address) {
                    info!("Found available port: {}", port);
                    return Ok(server);
                }
            }
        }
        // If we're not in development or couldn't find a free port, return original error
        Err(e)
    })?;

    // Log the actual bound address
    let addresses = server.addrs();
    info!("✨ Server ready:");
    for addr in addresses {
        info!("  🌎 http://{}", addr);
        info!("  🔧 Admin: http://{}/admin/stats", addr);
    }

    server.run().await
}
