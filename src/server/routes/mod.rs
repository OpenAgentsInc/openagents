pub mod chat;

use axum::Router;

pub fn routes() -> Router {
    Router::new()
        .nest("/chat", chat::routes())
}