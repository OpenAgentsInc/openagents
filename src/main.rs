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

use crate::event::Event;
use crate::relay::RelayWs;
use crate::db::Database;
use crate::configuration::get_configuration;
use sqlx::postgres::PgPoolOptions;

async fn ws_route(
    req: actix_web::HttpRequest,
    stream: web::Payload,
    event_tx: web::Data<broadcast::Sender<Event>>,
    db: web::Data<Arc<Database>>,
) -> Result<actix_web::HttpResponse, actix_web::Error> {
    let id = Uuid::new_v4().to_string();
    let ws = RelayWs::new(id, event_tx.get_ref().clone(), db.get_ref().clone());
    ws::start(ws, &req, stream)
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    env_logger::init();
    dotenv::dotenv().ok();

    // Load configuration
    let configuration = get_configuration().expect("Failed to read configuration.");
    
    // Initialize database using configuration
    let db = Arc::new(
        Database::new_with_options(configuration.database.connect_options())
            .await
            .expect("Failed to connect to database")
    );
    let db = web::Data::new(db);

    // Channel for broadcasting events to all connected clients
    let (event_tx, _): (broadcast::Sender<Event>, _) = broadcast::channel(1024);
    let event_tx = web::Data::new(event_tx);

    let address = format!(
        "{}:{}",
        configuration.application.host, configuration.application.port
    );

    HttpServer::new(move || {
        let cors = Cors::permissive();
        
        App::new()
            .wrap(cors)
            .app_data(event_tx.clone())
            .app_data(db.clone())
            .route("/", web::get().to(ws_route))
            .configure(server::config::configure_app)
    })
    .bind(&address)?
    .run()
    .await
}