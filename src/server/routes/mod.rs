use axum::Router;

pub mod chat;

pub fn routes() -> Router {
    Router::new()
        .nest("/chat", chat::chat_routes())
}