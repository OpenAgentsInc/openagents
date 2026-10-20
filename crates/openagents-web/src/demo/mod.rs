//! Compatibility addresses for the retired HTML demo.

use axum::{Router, response::Redirect, routing::get};

use crate::App;

pub(crate) fn routes() -> Router<App> {
    Router::new()
        .route("/demo", get(moved))
        .route("/demo/", get(moved))
        .route("/demo/{chat}", get(moved))
        .route("/demo/{chat}/thread", get(moved))
        .route("/demo/{chat}/message", get(moved).post(moved))
}

async fn moved() -> Redirect {
    Redirect::permanent("/demo/gpui")
}

#[cfg(test)]
mod tests;
