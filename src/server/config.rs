use actix_files as fs;
use actix_web::web;
use std::path::Path;

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
                    let path_str = path.to_str().unwrap_or("");
                    if path_str.ends_with(".js") {
                        Some(mime::APPLICATION_JAVASCRIPT)
                    } else {
                        None
                    }
                })
        );
}