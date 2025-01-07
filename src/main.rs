mod server;

use actix_web::{App, HttpServer};
use actix_cors::Cors;
use actix_web::middleware::{Logger, DefaultHeaders};
use dotenv::dotenv;
use log::info;

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    // Initialize environment
    dotenv().ok();
    env_logger::init();

    let port = std::env::var("PORT").unwrap_or_else(|_| "8080".to_string());
    let addr = format!("0.0.0.0:{}", port);
    
    info!("Starting server on {}", addr);
    
    HttpServer::new(|| {
        App::new()
            .wrap(
                Cors::default()
                    .allow_any_origin()
                    .allow_any_method()
                    .allow_any_header()
            )
            .wrap(Logger::default())
            .wrap(
                DefaultHeaders::new()
                    .add(("X-Content-Type-Options", "nosniff"))
                    .add(("X-Frame-Options", "DENY"))
                    .add(("X-XSS-Protection", "1; mode=block"))
            )
            .configure(server::config::configure_app)
    })
    .bind(&addr)?
    .run()
    .await
}