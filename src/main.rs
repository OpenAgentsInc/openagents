mod event;
mod relay;
mod subscription;

use actix_web::{web, App, HttpServer};
use actix_web_actors::ws;
use tokio::sync::broadcast;
use uuid::Uuid;

use crate::event::Event;
use crate::relay::RelayWs;

async fn ws_route(
    req: actix_web::HttpRequest,
    stream: web::Payload,
    event_tx: web::Data<broadcast::Sender<Event>>,
) -> Result<actix_web::HttpResponse, actix_web::Error> {
    let id = Uuid::new_v4().to_string();
    let ws = RelayWs::new(id, event_tx.get_ref().clone());
    ws::start(ws, &req, stream)
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    env_logger::init();

    // Channel for broadcasting events to all connected clients
    let (event_tx, _) = broadcast::channel(1024);
    let event_tx = web::Data::new(event_tx);

    HttpServer::new(move || {
        App::new()
            .app_data(event_tx.clone())
            .route("/", web::get().to(ws_route))
    })
    .bind("127.0.0.1:8080")?
    .run()
    .await
}