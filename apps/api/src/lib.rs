//! OpenAgents API — Cloudflare Worker (workers-rs).
//!
//! Run locally: `npx wrangler dev`
//! Deploy: `npx wrangler deploy`

use serde::{Deserialize, Serialize};
use worker::*;

#[derive(Debug, Deserialize, Serialize)]
struct ApiResponse<T> {
    ok: bool,
    data: Option<T>,
    error: Option<String>,
}

#[derive(Debug, Serialize)]
struct HealthData {
    status: &'static str,
    service: &'static str,
}

#[event(fetch)]
async fn main(req: Request, env: Env, _ctx: Context) -> Result<Response> {
    Router::new()
        .get_async("/", handle_root)
        .get_async("/health", handle_health)
        .run(req, env)
        .await
}

async fn handle_root(_: Request, _: RouteContext<()>) -> Result<Response> {
    Response::from_json(&ApiResponse {
        ok: true,
        data: Some(serde_json::json!({
            "name": "openagents-api",
            "docs": "https://github.com/cloudflare/workers-rs"
        })),
        error: None,
    })
}

async fn handle_health(_: Request, _: RouteContext<()>) -> Result<Response> {
    Response::from_json(&ApiResponse {
        ok: true,
        data: Some(HealthData {
            status: "ok",
            service: "openagents-api",
        }),
        error: None,
    })
}
