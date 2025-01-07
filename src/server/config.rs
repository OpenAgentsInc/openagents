use actix_files as fs;
use actix_web::web;
use actix_web::middleware::DefaultHeaders;

use super::routes;

pub fn configure_app(cfg: &mut web::ServiceConfig) {
    // Configure routes
    cfg.service(routes::health_check)
        // Add MIME type headers for JavaScript files
        .service(
            fs::Files::new("/", "./static")
                .index_file("index.html")
                .use_hidden_files()
                .prefer_utf8(true)
                .default_handler(|req: actix_files::NamedFile| {
                    if req.path().ends_with(".js") {
                        req.set_content_type("application/javascript")
                    }
                    Ok(req)
                })
        );
}