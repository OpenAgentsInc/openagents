use axum::{
    extract::FromRef,
    routing::{get, post},
    Router,
};
use std::{env, sync::Arc};
use sqlx::PgPool;
use tower_http::services::ServeDir;

use crate::routes;
use super::{
    services::{
        deepseek::DeepSeekService,
        github_auth::{GitHubAuthService, GitHubConfig},
        github_issue::GitHubService,
        RepomapService,
    },
    tools::create_tools,
    ws::transport::WebSocketState,
    handlers::auth::AuthState,
    hyperview,
    ws,
    handlers,
};