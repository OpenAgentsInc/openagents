use actix_files as fs;
use actix_web::web;

use super::{admin::middleware::AdminAuth, routes};

pub fn configure_app(cfg: &mut web::ServiceConfig) {
    // Configure admin routes with authentication
    cfg.service(
        web::scope("/admin")
            .wrap(AdminAuth::new())
            .configure(crate::server::admin::routes::admin_config),
    );

    // Configure non-admin routes
    routes::configure_routes(cfg);

    // Serve static files from the static directory
    cfg.service(
        fs::Files::new("/", "./static")
            .index_file("index.html")
            .use_hidden_files()
            .prefer_utf8(true)
            .show_files_listing(),
    );
}
