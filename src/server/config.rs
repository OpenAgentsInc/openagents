use actix_files as fs;
use actix_web::web;
use mime_guess::from_path;

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
                    Some(from_path(path).first_or_octet_stream())
                })
        );
}