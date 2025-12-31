//! OpenAgents Web Worker - Axum API on Cloudflare Workers
//!
//! Handles API routes, GitHub OAuth, Stripe payments, and serves WGPUI frontend.

use worker::*;

mod agent_do;
mod autopilot_container;
mod db;
mod identity;
mod middleware;
mod relay;
mod routes;
mod services;

pub use agent_do::AgentDo;
pub use autopilot_container::AutopilotContainer;
pub use db::sessions::Session;
pub use middleware::auth::AuthenticatedUser;

/// Main worker entry point
#[event(fetch)]
async fn fetch(mut req: Request, env: Env, _ctx: Context) -> Result<Response> {
    console_error_panic_hook::set_once();

    // Get request info
    let url = req.url()?;
    let path = url.path();
    let method = req.method();

    // Route the request
    match (method, path.as_ref()) {
        // GitHub Auth routes
        (Method::Get, "/api/auth/github/start") => routes::auth::github_start(req, env).await,
        (Method::Get, "/api/auth/github/callback") => routes::auth::github_callback(req, env).await,
        (Method::Post, "/api/auth/logout") => routes::auth::logout(req, env).await,

        // Claude Auth routes (OAuth with PKCE)
        (Method::Get, "/api/auth/claude/start") => routes::claude_auth::claude_start(req, env).await,
        (Method::Get, "/api/auth/claude/callback") => routes::claude_auth::claude_callback(req, env).await,
        (Method::Get, "/api/auth/claude/status") => routes::claude_auth::claude_status(req, env).await,
        (Method::Post, "/api/auth/claude/disconnect") => routes::claude_auth::claude_disconnect(req, env).await,
        (Method::Post, "/api/auth/claude/link") => {
            let body = req.text().await?;
            routes::claude_auth::claude_link(req, env, body).await
        }
        (Method::Get, "/api/auth/me") => {
            // Return current user or null
            match get_optional_user(&req, &env).await {
                Some(user) => {
                    let db = env.d1("DB")?;
                    let user_record = db::users::get_by_id(&db, &user.user_id).await?;
                    Response::from_json(&serde_json::json!({
                        "user_id": user_record.user_id,
                        "github_username": user_record.github_username,
                        "nostr_npub": user_record.nostr_npub,
                    }))
                }
                None => Response::from_json(&serde_json::json!({ "user": null })),
            }
        }

        // Repos route (require auth)
        (Method::Get, "/api/repos") => {
            let env_clone = env.clone();
            with_auth(&req, &env, |user| async move {
                let db = env_clone.d1("DB")?;
                let session_secret = env_clone.secret("SESSION_SECRET")?.to_string();
                let access_token =
                    db::users::get_github_access_token(&db, &user.user_id, &session_secret).await?;

                let repos = services::github::get_repos(&access_token).await?;

                Response::from_json(&repos)
            }).await
        }

        // Account routes (require auth)
        (Method::Get, "/api/account") => {
            with_auth(&req, &env, |user| routes::account::get_settings(user, env.clone())).await
        }
        (Method::Post, "/api/account/api-key") => {
            with_auth(&req, &env, |user| routes::account::generate_api_key(user, env.clone())).await
        }
        (Method::Post, "/api/account/delete") => {
            with_auth(&req, &env, |user| routes::account::delete_account(user, env.clone())).await
        }

        // Agent routes (require auth)
        (Method::Get, "/api/agents") => {
            with_auth(&req, &env, |user| routes::agents::list(user, env.clone())).await
        }
        (Method::Post, "/api/agents") => {
            let body = req.text().await?;
            with_auth(&req, &env, |user| {
                routes::agents::create(user, env.clone(), body.clone())
            })
            .await
        }
        (Method::Get, path) if path.starts_with("/api/agents/") => {
            let segments: Vec<&str> = path.trim_start_matches("/api/agents/").split('/').collect();
            if segments.len() == 1 {
                let agent_id = parse_agent_id(segments[0])?;
                with_auth(&req, &env, |user| {
                    routes::agents::get(user, env.clone(), agent_id)
                })
                .await
            } else if segments.len() == 3 && segments[1] == "do" && segments[2] == "status" {
                let agent_id = parse_agent_id(segments[0])?;
                with_auth(&req, &env, |user| {
                    routes::agents::do_status(user, env.clone(), agent_id)
                })
                .await
            } else {
                Response::error("Invalid agent path", 400)
            }
        }
        (Method::Post, path) if path.starts_with("/api/agents/") => {
            let segments: Vec<&str> = path.trim_start_matches("/api/agents/").split('/').collect();
            if segments.len() == 3 && segments[1] == "do" && segments[2] == "tick" {
                let agent_id = parse_agent_id(segments[0])?;
                with_auth(&req, &env, |user| {
                    routes::agents::do_tick(user, env.clone(), agent_id)
                })
                .await
            } else {
                Response::error("Invalid agent path", 400)
            }
        }
        (Method::Delete, path) if path.starts_with("/api/agents/") => {
            let segments: Vec<&str> = path.trim_start_matches("/api/agents/").split('/').collect();
            if segments.len() == 1 {
                let agent_id = parse_agent_id(segments[0])?;
                with_auth(&req, &env, |user| {
                    routes::agents::delete(user, env.clone(), agent_id)
                })
                .await
            } else {
                Response::error("Invalid agent path", 400)
            }
        }

        // Billing routes (require auth)
        (Method::Get, "/api/billing/balance") => {
            with_auth(&req, &env, |user| routes::billing::get_balance(user, env.clone())).await
        }
        (Method::Get, "/api/billing/plans") => routes::billing::list_plans().await,
        (Method::Get, "/api/billing/credits") => routes::billing::list_credit_packages().await,
        (Method::Post, "/api/billing/credits/purchase") => {
            let body = req.text().await?;
            with_auth(&req, &env, |user| {
                routes::billing::purchase_credits(user, env.clone(), body.clone())
            })
            .await
        }

        // Wallet routes - redirect to wallet-worker
        // Service binding doesn't cleanly convert response types, so we redirect
        (_, path) if path.starts_with("/api/wallet") => {
            let wallet_url = format!("https://openagents-wallet.openagents.workers.dev{}", path);
            let parsed_url = worker::Url::parse(&wallet_url)?;
            Response::redirect_with_status(parsed_url, 307)
        }

        // Stripe routes
        (Method::Get, "/api/stripe/config") => routes::stripe::get_config(env).await,
        (Method::Get, "/api/stripe/payment-methods") => {
            with_auth(&req, &env, |user| {
                routes::stripe::list_payment_methods(user, env.clone())
            })
            .await
        }
        (Method::Post, "/api/stripe/setup-intent") => {
            with_auth(&req, &env, |user| {
                routes::stripe::create_setup_intent(user, env.clone())
            })
            .await
        }
        (Method::Post, "/webhooks/stripe") => {
            let signature = req
                .headers()
                .get("stripe-signature")?
                .unwrap_or_default();
            let body = req.bytes().await?;
            routes::stripe::webhook(env, signature, body).await
        }

        // Embed route: /repo/:username/:repo/embed
        (Method::Get, path) if path.starts_with("/repo/") && path.ends_with("/embed") => {
            let inner = path.trim_start_matches("/repo/").trim_end_matches("/embed");
            let parts: Vec<&str> = inner.split('/').collect();
            if parts.len() >= 2 {
                let username = parts[0].to_string();
                let repo = parts[1..].join("/");
                let agent_id = url
                    .query_pairs()
                    .find(|(k, _)| k == "agent_id")
                    .map(|(_, v)| v.to_string());
                let stream_override = url
                    .query_pairs()
                    .find(|(k, _)| k == "stream")
                    .map(|(_, v)| v.to_string());
                routes::hud::embed_hud(env, username, repo, agent_id, stream_override).await
            } else {
                Response::error("Invalid embed path", 400)
            }
        }
        // Embed route alias: /hud/@username/:repo/embed
        (Method::Get, path) if path.starts_with("/hud/") && path.ends_with("/embed") => {
            let inner = path.trim_start_matches("/hud/").trim_end_matches("/embed");
            let parts: Vec<&str> = inner.split('/').collect();
            if parts.len() >= 2 {
                let mut username = parts[0].to_string();
                if let Some(stripped) = username.strip_prefix('@') {
                    username = stripped.to_string();
                }
                let repo = parts[1..].join("/");
                let agent_id = url
                    .query_pairs()
                    .find(|(k, _)| k == "agent_id")
                    .map(|(_, v)| v.to_string());
                let stream_override = url
                    .query_pairs()
                    .find(|(k, _)| k == "stream")
                    .map(|(_, v)| v.to_string());
                routes::hud::embed_hud(env, username, repo, agent_id, stream_override).await
            } else {
                Response::error("Invalid embed path", 400)
            }
        }
        // Repo routes (GTM)
        (Method::Get, path) if path.starts_with("/repo/") => {
            let parts: Vec<&str> = path.trim_start_matches("/repo/").split('/').collect();
            if parts.len() >= 2 {
                let username = parts[0].to_string();
                let repo = parts[1..].join("/");
                let maybe_user = get_optional_user(&req, &env).await;
                let agent_id = url
                    .query_pairs()
                    .find(|(k, _)| k == "agent_id")
                    .map(|(_, v)| v.to_string());
                let stream_override = url
                    .query_pairs()
                    .find(|(k, _)| k == "stream")
                    .map(|(_, v)| v.to_string());
                routes::hud::view_hud(
                    env,
                    username,
                    repo,
                    maybe_user,
                    agent_id,
                    stream_override,
                )
                .await
            } else {
                Response::error("Invalid HUD path", 400)
            }
        }
        // HUD route alias: /hud/@username/:repo
        (Method::Get, path) if path.starts_with("/hud/") => {
            let parts: Vec<&str> = path.trim_start_matches("/hud/").split('/').collect();
            if parts.len() >= 2 {
                let mut username = parts[0].to_string();
                if let Some(stripped) = username.strip_prefix('@') {
                    username = stripped.to_string();
                }
                let repo = parts[1..].join("/");
                let maybe_user = get_optional_user(&req, &env).await;
                let agent_id = url
                    .query_pairs()
                    .find(|(k, _)| k == "agent_id")
                    .map(|(_, v)| v.to_string());
                let stream_override = url
                    .query_pairs()
                    .find(|(k, _)| k == "stream")
                    .map(|(_, v)| v.to_string());
                routes::hud::view_hud(
                    env,
                    username,
                    repo,
                    maybe_user,
                    agent_id,
                    stream_override,
                )
                .await
            } else {
                Response::error("Invalid HUD path", 400)
            }
        }
        (Method::Post, "/api/hud/settings") => {
            let body = req.text().await?;
            with_auth(&req, &env, |user| {
                routes::hud::update_settings(user, env.clone(), body.clone())
            })
            .await
        }
        (Method::Get, "/api/hud/settings") => {
            let repo = url
                .query_pairs()
                .find(|(k, _)| k == "repo")
                .map(|(_, v)| v.to_string())
                .unwrap_or_default();
            if repo.is_empty() {
                return Response::error("Missing repo", 400);
            }
            with_auth(&req, &env, |user| {
                routes::hud::get_settings(user, env.clone(), repo.clone())
            })
            .await
        }
        (Method::Get, "/api/hud/live") => routes::hud::live_hud(env).await,
        (Method::Get, "/api/hud/session") => {
            let repo = url.query_pairs()
                .find(|(k, _)| k == "repo")
                .map(|(_, v)| v.to_string())
                .unwrap_or_default();
            with_auth(&req, &env, |user| {
                routes::hud::get_session(user, env.clone(), repo.clone())
            })
            .await
        }
        (Method::Post, "/api/hud/start") => {
            let body = req.text().await?;
            with_auth(&req, &env, |user| {
                routes::hud::start_session(user, env.clone(), body.clone())
            })
            .await
        }

        // Funnel analytics events
        (Method::Post, "/api/analytics/event") => {
            let body = req.text().await?;
            let maybe_user = get_optional_user(&req, &env).await;
            routes::analytics::track_event(env, maybe_user, body).await
        }

        // Tunnel routes (WebSocket relay)
        (Method::Post, "/api/tunnel/register") => {
            let body = req.text().await?;
            let origin = req.url()?.origin().ascii_serialization();
            let env_clone = env.clone();
            with_auth(&req, &env, |user| async move {
                routes::tunnel::register_with_origin(env_clone, user, body, origin).await
            })
            .await
        }
        (Method::Get, path) if path.starts_with("/api/tunnel/status/") => {
            let session_id = path.trim_start_matches("/api/tunnel/status/").to_string();
            routes::tunnel::status(env, session_id).await
        }
        (Method::Get, path) if path.starts_with("/api/tunnel/ws/") => {
            routes::tunnel::websocket(req, env).await
        }

        // Container routes (paid tier - cloud compute)
        (Method::Post, "/api/container/start") => {
            let body = req.text().await?;
            with_auth(&req, &env, |user| {
                routes::container::start_task(user, env.clone(), body.clone())
            })
            .await
        }
        (Method::Get, "/api/container/status") => {
            with_auth(&req, &env, |user| {
                routes::container::get_status(user, env.clone())
            })
            .await
        }
        (Method::Get, path) if path.starts_with("/api/container/ws/") => {
            routes::container::websocket(req, env).await
        }

        // For all other routes, let Cloudflare's asset binding handle it
        // This includes /, /index.html, /pkg/*, etc.
        _ => Response::error("Not Found", 404),
    }
}

/// Authenticate request and run handler
async fn with_auth<F, Fut>(req: &Request, env: &Env, handler: F) -> Result<Response>
where
    F: FnOnce(AuthenticatedUser) -> Fut,
    Fut: std::future::Future<Output = Result<Response>>,
{
    match middleware::auth::authenticate(req, env).await {
        Ok(user) => handler(user).await,
        Err(e) => Response::error(format!("Unauthorized: {}", e), 401),
    }
}

/// Get user if authenticated, None otherwise
async fn get_optional_user(req: &Request, env: &Env) -> Option<AuthenticatedUser> {
    middleware::auth::authenticate(req, env).await.ok()
}

fn parse_agent_id(segment: &str) -> Result<i64> {
    segment
        .parse::<i64>()
        .map_err(|_| Error::RustError("Invalid agent id".to_string()))
}
