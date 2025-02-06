use axum::{
    Router,
    routing::get,
};
use super::handlers;

pub fn hyperview_routes() -> Router {
    Router::new()
        .route("/hyperview", get(handlers::hello_world))
}