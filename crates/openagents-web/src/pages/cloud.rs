//! `/cloud`: the former public Cloud page now redirects home. The signed-in
//! Cloud app (`/cloud/app` and below, [`crate::cloud`]) is unchanged.

use axum::Router;
use axum::response::{IntoResponse, Redirect};
use axum::routing::get;

use crate::App;

pub(crate) fn routes() -> Router<App> {
    Router::new().route(
        "/cloud",
        get(|| async { Redirect::to("/").into_response() }),
    )
}
