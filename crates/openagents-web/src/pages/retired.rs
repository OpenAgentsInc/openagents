//! Compatibility redirects for retired pages. Shared builds and APIs stay mounted.

use crate::App;
use axum::{Router, extract::OriginalUri, response::Redirect, routing::get};

pub(crate) fn routes() -> Router<App> {
    Router::new()
        .route("/lander3", get(home))
        .route("/lander3/", get(home))
        .route("/lander3/{*rest}", get(home))
        .route("/lander4", get(home))
        .route("/lander4/", get(home))
        .route("/lander4/{*rest}", get(home))
        .route("/lander5", get(home))
        .route("/lander5/", get(home))
        .route("/lander5/{*rest}", get(home))
        .route("/lander6", get(home))
        .route("/lander6/", get(home))
        .route("/lander6/{*rest}", get(home))
        .route("/lander7", get(home))
        .route("/lander7/", get(home))
        .route("/lander7/{*rest}", get(home))
        .route("/lander8", get(home))
        .route("/lander8/", get(home))
        .route("/lander8/chat/{id}", get(chat))
        .route("/lander9", get(home))
        .route("/lander9/", get(home))
        .route("/lander9/{*rest}", get(lander9))
        .route("/landing", get(home))
        .route("/landing/", get(home))
        .route("/landing/{*rest}", get(home))
}

async fn home() -> Redirect {
    Redirect::permanent("/")
}

async fn chat(OriginalUri(uri): OriginalUri) -> Redirect {
    let path = uri.path().strip_prefix("/lander8").unwrap_or("/");
    let to = match uri.query() {
        Some(query) => format!("{path}?{query}"),
        None => path.to_owned(),
    };
    Redirect::permanent(&to)
}

async fn lander9(OriginalUri(uri): OriginalUri) -> Redirect {
    match uri.path().strip_prefix("/lander9/hands/") {
        Some(file) => Redirect::permanent(&format!("/verse/hands/{file}")),
        None => Redirect::permanent("/"),
    }
}
