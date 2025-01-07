use actix_files as fs;
use actix_web::web;
use actix_web::http::header::ContentType;
use actix_web::{HttpResponse, dev::ServiceResponse};
use actix_service::ServiceFactory;

use super::routes;

pub fn configure_app(cfg: &mut web::ServiceConfig) {
    // Configure routes
    cfg.service(routes::health_check)
        // Serve static files from the static directory
        .service(
            fs::Files::new("/", "./static")
                .index_file("index.html")
                .use_hidden_files()
                .prefer_utf8(true)
                .mime_override(|path| {
                    if path.ends_with(".js") {
                        Some(mime::APPLICATION_JAVASCRIPT)
                    } else {
                        None
                    }
                })
        );
}