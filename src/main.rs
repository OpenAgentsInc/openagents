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

    info!("Starting server...");
    
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
            .configure(crate::server::config::configure_app)
    })
    .bind(("127.0.0.1", 8080))?
    .run()
    .await
}