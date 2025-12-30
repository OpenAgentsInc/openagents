//! OpenAgents Web Worker - Axum API on Cloudflare Workers
//!
//! Handles API routes, GitHub OAuth, Stripe payments, and serves WGPUI frontend.

use worker::*;

mod db;
mod middleware;
mod routes;
mod services;

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
        // Auth routes
        (Method::Get, "/api/auth/github/start") => routes::auth::github_start(req, env).await,
        (Method::Get, "/api/auth/github/callback") => routes::auth::github_callback(req, env).await,
        (Method::Post, "/api/auth/logout") => routes::auth::logout(req, env).await,
        (Method::Get, "/api/auth/me") => {
            // Return current user or null
            match get_optional_user(&req, &env).await {
                Some(user) => Response::from_json(&serde_json::json!({
                    "user_id": user.user_id,
                    "github_username": user.github_username,
                })),
                None => Response::from_json(&serde_json::json!({ "user": null })),
            }
        }

        // Repos route (require auth)
        (Method::Get, "/api/repos") => {
            let env_clone = env.clone();
            with_auth(&req, &env, |user| async move {
                let db = env_clone.d1("DB")?;
                let user_record = db::users::get_by_id(&db, &user.user_id).await?;

                // Get GitHub access token (stored during OAuth)
                let access_token = user_record.github_access_token
                    .ok_or_else(|| Error::RustError("No GitHub token".to_string()))?;

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

        // Repo routes (GTM)
        (Method::Get, path) if path.starts_with("/repo/") => {
            let parts: Vec<&str> = path.trim_start_matches("/repo/").split('/').collect();
            if parts.len() >= 2 {
                let username = parts[0].to_string();
                let repo = parts[1..].join("/");
                let maybe_user = get_optional_user(&req, &env).await;
                routes::hud::view_hud(env, username, repo, maybe_user).await
            } else {
                Response::error("Invalid HUD path", 400)
            }
        }
        (Method::Get, path) if path.starts_with("/embed/") => {
            let parts: Vec<&str> = path.trim_start_matches("/embed/").split('/').collect();
            if parts.len() >= 2 {
                let username = parts[0].to_string();
                let repo = parts[1..].join("/");
                routes::hud::embed_hud(env, username, repo).await
            } else {
                Response::error("Invalid embed path", 400)
            }
        }
        (Method::Post, "/api/hud/settings") => {
            let body = req.text().await?;
            with_auth(&req, &env, |user| {
                routes::hud::update_settings(user, env.clone(), body.clone())
            })
            .await
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
