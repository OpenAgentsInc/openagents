use actix_cors::Cors;
use actix_files as fs;
use actix_web::web;
use actix_web::middleware::{Logger, DefaultHeaders};

use super::routes;

pub fn configure_app(cfg: &mut web::ServiceConfig) {
    // Configure routes
    cfg.service(routes::health_check)
        // Serve static files from the static directory
        .service(
            fs::Files::new("/", "./static")
                .index_file("index.html")
                .use_hidden_files()
        )
        // Add middleware
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
        );
}