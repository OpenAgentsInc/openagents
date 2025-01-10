mod event;
mod relay;
mod subscription;
mod db;
mod server;

use actix_web::{web, App, HttpServer};
use actix_web_actors::ws;
use actix_cors::Cors;
use tokio::sync::broadcast;
use uuid::Uuid;
use std::sync::Arc;

use crate::event::Event;
use crate::relay::RelayWs;
use crate::db::Database;

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

    // Initialize database
    let database_url = std::env::var("DATABASE_URL")
        .expect("DATABASE_URL must be set");
    let db = Arc::new(Database::new(&database_url).await
        .expect("Failed to connect to database"));
    let db = web::Data::new(db);

    // Channel for broadcasting events to all connected clients
    let (event_tx, _): (broadcast::Sender<Event>, _) = broadcast::channel(1024);
    let event_tx = web::Data::new(event_tx);

    HttpServer::new(move || {
        let cors = Cors::permissive();
        
        App::new()
            .wrap(cors)
            .app_data(event_tx.clone())
            .app_data(db.clone())
            .route("/", web::get().to(ws_route))
            .configure(server::config::configure_app)
    })
    .bind("127.0.0.1:8080")?
    .run()
    .await
}
