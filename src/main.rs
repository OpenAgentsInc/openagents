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

    let base_port = std::env::var("PORT").unwrap_or_else(|_| "8080".to_string()).parse::<u16>().unwrap();
    let max_retries = 10;
    
    let mut last_error = None;
    
    for port in base_port..base_port+max_retries {
        let addr = format!("0.0.0.0:{}", port);
        info!("Attempting to start server on {}", addr);
        
        match HttpServer::new(|| {
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
        .bind(&addr)
        {
            Ok(server) => {
                println!("\n🚀 Server running!");
                println!("➜ Local:   \x1b[36mhttp://localhost:{}\x1b[0m", port);
                println!("➜ Network: \x1b[36mhttp://0.0.0.0:{}\x1b[0m\n", port);
                return server.run().await;
            }
            Err(e) => {
                info!("Failed to bind to port {}: {}", port, e);
                last_error = Some(e);
                continue;
            }
        }
    }
    
    Err(last_error.unwrap_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::AddrInUse,
            format!("Failed to bind to any port in range {}-{}", base_port, base_port+max_retries-1)
        )
    }))
}