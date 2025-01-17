use actix_files as fs;
use actix_web::web;
use std::env;

use super::{admin::middleware::AdminAuth, routes, services::RepomapService};

pub fn configure_app(cfg: &mut web::ServiceConfig) {
    // Initialize repomap service
    let aider_api_key = env::var("AIDER_API_KEY").unwrap_or_else(|_| "".to_string());
    let repomap_service = RepomapService::new(aider_api_key);
    
    // Add repomap service to app data
    cfg.app_data(web::Data::new(repomap_service));

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
        fs::Files::new("/static", "./static")
            .show_files_listing()
            .use_last_modified(true),
    );

    // Serve template files
    cfg.service(
        fs::Files::new("/templates", "./templates")
            .show_files_listing()
            .use_last_modified(true),
    );
}