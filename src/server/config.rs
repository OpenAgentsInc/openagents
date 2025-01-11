use actix_files as fs;
use actix_web::web;

use super::{routes, admin::middleware::AdminAuth};

pub fn configure_app(cfg: &mut web::ServiceConfig) {
    // Configure admin routes with authentication
    cfg.service(
        web::scope("/admin")
            .wrap(AdminAuth::new())
            .configure(crate::server::admin::routes::admin_config)
    );

    // Configure non-admin routes
    cfg.service(routes::health_check)
        .service(routes::new_page)
        // Serve static files from the static directory
        .service(
            fs::Files::new("/", "./static")
                .index_file("index.html")
                .use_hidden_files()
                .prefer_utf8(true)
                .show_files_listing()
        );
}
