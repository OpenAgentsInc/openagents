use actix_files as fs;
use actix_web::web;
use tracing::info;
use std::fs as std_fs;

use super::{routes, admin::middleware::AdminAuth};

pub fn configure_app(cfg: &mut web::ServiceConfig) {
    // Log static directory contents
    info!("Checking static directory contents:");
    if let Ok(entries) = std_fs::read_dir("./static") {
        for entry in entries {
            if let Ok(entry) = entry {
                info!("  {:?}", entry.path());
            }
        }
    } else {
        info!("Could not read ./static directory");
    }

    if let Ok(entries) = std_fs::read_dir("/app/static") {
        info!("Checking /app/static directory contents:");
        for entry in entries {
            if let Ok(entry) = entry {
                info!("  {:?}", entry.path());
            }
        }
    } else {
        info!("Could not read /app/static directory");
    }

    // Configure admin routes with authentication
    cfg.service(
        web::scope("/admin")
            .wrap(AdminAuth::new())
            .configure(routes::admin::routes::admin_config)
    );

    // Configure non-admin routes
    cfg.service(routes::health_check)
        // Serve static files from the static directory
        .service(
            fs::Files::new("/", "/app/static")
                .index_file("index.html")
                .use_hidden_files()
                .prefer_utf8(true)
                .show_files_listing()
        );
}
=======
    // Log static directory contents
    info!("Checking static directory contents:");
    if let Ok(entries) = std_fs::read_dir("./static") {
        for entry in entries {
            if let Ok(entry) = entry {
                info!("  {:?}", entry.path());
            }
        }
    } else {
        info!("Could not read ./static directory");
    }

    if let Ok(entries) = std_fs::read_dir("/app/static") {
        info!("Checking /app/static directory contents:");
        for entry in entries {
            if let Ok(entry) = entry {
                info!("  {:?}", entry.path());
            }
        }
    } else {
        info!("Could not read /app/static directory");
    }

    // Configure routes
>>>>>>> origin/main
    cfg.service(routes::health_check)
        // Serve static files from the static directory
        .service(
            fs::Files::new("/", "/app/static")
                .index_file("index.html")
                .use_hidden_files()
                .prefer_utf8(true)
                .show_files_listing()
        );
}
