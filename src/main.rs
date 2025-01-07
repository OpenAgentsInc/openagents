mod server;

use actix_web::{App, HttpServer};
use dotenv::dotenv;
use log::info;

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    // Initialize environment
    dotenv().ok();
    env_logger::init();

    info!("Starting server...");
    
    HttpServer::new(|| {
        App::new()
            .configure(server::config::configure_app)
    })
    .bind(("127.0.0.1", 8080))?
    .run()
    .await
}