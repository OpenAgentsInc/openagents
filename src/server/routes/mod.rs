use actix_web::web;

pub mod repomap;

pub fn configure_routes(cfg: &mut web::ServiceConfig) {
    cfg.service(repomap::get_repomap)
       .service(repomap::generate_repomap);
}