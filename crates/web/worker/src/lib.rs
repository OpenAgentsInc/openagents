//! OpenAgents Web Worker - Axum API on Cloudflare Workers
//!
//! Handles API routes, GitHub OAuth, Stripe payments, and serves WGPUI frontend.

#![allow(dead_code)]

use worker::*;

mod agent_do;
mod autopilot_container;
mod db;
mod identity;
mod middleware;
mod relay;
mod rlm_do;
mod routes;
mod services;

pub use agent_do::AgentDo;
pub use autopilot_container::AutopilotContainer;
pub use db::sessions::Session;
pub use middleware::auth::AuthenticatedUser;
pub use rlm_do::RlmRunDO;

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

        // GitHub explore route (require auth)
        (Method::Get, "/api/github/explore") => {
            routes::github_explore::explore(req, env).await
        }
        (Method::Get, "/api/github/contents") => {
            routes::github_explore::contents(req, env).await
        }

        // Repo knowledge routes (agent memory persistence)
        (Method::Get, path) if path.starts_with("/api/repo-knowledge/") => {
            let parts: Vec<&str> = path.trim_start_matches("/api/repo-knowledge/").split('/').collect();
            if parts.len() == 2 {
                routes::repo_knowledge::get_knowledge(req, env, parts[0], parts[1]).await
            } else {
                Response::error("Invalid path, expected /api/repo-knowledge/:owner/:repo", 400)
            }
        }
        (Method::Post, path) if path.starts_with("/api/repo-knowledge/") => {
            let parts: Vec<&str> = path.trim_start_matches("/api/repo-knowledge/").split('/').collect();
            if parts.len() == 2 {
                routes::repo_knowledge::save_knowledge(req, env, parts[0], parts[1]).await
            } else {
                Response::error("Invalid path, expected /api/repo-knowledge/:owner/:repo", 400)
            }
        }
        (Method::Post, path) if path.starts_with("/api/file-knowledge/") => {
            let parts: Vec<&str> = path.trim_start_matches("/api/file-knowledge/").split('/').collect();
            if parts.len() == 2 {
                routes::repo_knowledge::save_file_knowledge(req, env, parts[0], parts[1]).await
            } else {
                Response::error("Invalid path, expected /api/file-knowledge/:owner/:repo", 400)
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
        // RLM routes (require auth except sync)
        (Method::Get, "/api/rlm/runs") => {
            let url = req.url()?;
            let mut limit = 20u32;
            let mut before = None;
            for (key, value) in url.query_pairs() {
                match key.as_ref() {
                    "limit" => limit = value.parse::<u32>().unwrap_or(20).min(100),
                    "before" => before = value.parse::<i64>().ok(),
                    _ => {}
                }
            }
            with_auth(&req, &env, |user| {
                routes::rlm::list_runs(user, env.clone(), limit, before)
            })
            .await
        }
        (Method::Get, "/api/rlm/experiments") => {
            let url = req.url()?;
            let mut limit = 20u32;
            let mut before = None;
            for (key, value) in url.query_pairs() {
                match key.as_ref() {
                    "limit" => limit = value.parse::<u32>().unwrap_or(20).min(100),
                    "before" => before = value.parse::<i64>().ok(),
                    _ => {}
                }
            }
            with_auth(&req, &env, |user| {
                routes::rlm::list_experiments(user, env.clone(), limit, before)
            })
            .await
        }
        (Method::Post, "/api/rlm/experiments") => {
            let body = req.text().await?;
            with_auth(&req, &env, |user| {
                routes::rlm::create_experiment(user, env.clone(), body.clone())
            })
            .await
        }
        (Method::Post, "/api/rlm/runs/sync") => {
            let body = req.text().await?;
            routes::rlm::sync(env, body).await
        }
        (Method::Get, path) if path.starts_with("/api/rlm/runs/") => {
            let segments: Vec<&str> = path.trim_start_matches("/api/rlm/runs/").split('/').collect();
            if segments.len() == 1 {
                let run_id = segments[0].to_string();
                with_auth(&req, &env, |user| {
                    routes::rlm::get_run(user, env.clone(), run_id.clone())
                })
                .await
            } else if segments.len() == 2 && segments[1] == "trace" {
                let run_id = segments[0].to_string();
                with_auth(&req, &env, |user| {
                    routes::rlm::get_trace(user, env.clone(), run_id.clone())
                })
                .await
            } else {
                Response::error("Invalid RLM path", 400)
            }
        }
        (Method::Get, "/api/rlm/providers") => {
            let url = req.url()?;
            let mut limit = 2000u32;
            for (key, value) in url.query_pairs() {
                if key == "limit" {
                    limit = value.parse::<u32>().unwrap_or(2000).min(10000);
                }
            }
            with_auth(&req, &env, |user| {
                routes::rlm::list_providers(user, env.clone(), limit)
            })
            .await
        }
        (method, path) if path.starts_with("/api/rlm/experiments/") => {
            let segments: Vec<&str> = path.trim_start_matches("/api/rlm/experiments/").split('/').collect();
            if segments.is_empty() || segments[0].is_empty() {
                return Response::error("Missing experiment id", 400);
            }
            let experiment_id = segments[0].to_string();
            match (method, segments.get(1).copied(), segments.get(2).copied()) {
                (Method::Get, None, None) => {
                    with_auth(&req, &env, |user| {
                        routes::rlm::get_experiment(user, env.clone(), experiment_id.clone())
                    })
                    .await
                }
                (Method::Post, Some("runs"), None) => {
                    let body = req.text().await?;
                    with_auth(&req, &env, |user| {
                        routes::rlm::add_experiment_runs(
                            user,
                            env.clone(),
                            experiment_id.clone(),
                            body.clone(),
                        )
                    })
                    .await
                }
                (Method::Delete, Some("runs"), Some(run_id)) => {
                    with_auth(&req, &env, |user| {
                        routes::rlm::remove_experiment_run(
                            user,
                            env.clone(),
                            experiment_id.clone(),
                            run_id.to_string(),
                        )
                    })
                    .await
                }
                (Method::Get, Some("export"), None) => {
                    with_auth(&req, &env, |user| {
                        let format = req
                            .url()
                            .ok()
                            .and_then(|url| url.query_pairs().find(|(k, _)| k == "format").map(|(_, v)| v.to_string()));
                        let format = routes::export::ExportFormat::from_param(format);
                        routes::export::export_experiment(user, env.clone(), experiment_id.clone(), format)
                    })
                    .await
                }
                _ => Response::error("Invalid experiment path", 400),
            }
        }
        (Method::Get, path) if path.starts_with("/api/rlm/ws/") => {
            routes::rlm::websocket(req, env).await
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

        // GFN (Group Forming Networks) page - public educational page
        (Method::Get, "/gfn") => routes::gfn::view_gfn(env).await,

        // FRLM (Fracking Apple Silicon) power comparison page
        (Method::Get, "/frack") => routes::frlm::view_frlm(env).await,
        // Episode 201 transcript
        (Method::Get, "/fracking-apple-silicon") => {
            routes::fracking_apple_silicon::view_fracking_apple_silicon(env).await
        }

        // ML inference visualization page
        (Method::Get, "/ml-inference") => routes::ml_inference::view_ml_inference(env).await,

        // GPT-OSS pipeline visualization page
        (Method::Get, "/gptoss") => routes::gptoss::view_gptoss(env).await,

        // RLM (Recursive Language Model) dashboard
        (Method::Get, "/rlm") => routes::rlm::view_rlm(env).await,
        (Method::Get, "/rlm/demo") => routes::rlm::view_rlm_demo(env).await,
        (Method::Get, "/rlm/runs") => routes::rlm::view_rlm(env).await,
        (Method::Get, path) if path.starts_with("/rlm/runs/") => {
            let run_id = path.trim_start_matches("/rlm/runs/").to_string();
            routes::rlm::view_rlm_detail(env, run_id).await
        }
        (Method::Get, "/rlm/experiments") => routes::rlm::view_rlm_experiments(env).await,
        (Method::Get, path) if path.starts_with("/rlm/experiments/") => {
            let experiment_id = path.trim_start_matches("/rlm/experiments/").to_string();
            routes::rlm::view_rlm_experiment_detail(env, experiment_id).await
        }
        (Method::Get, "/rlm/providers") => routes::rlm::view_rlm_providers(env).await,

        // FM Bridge (Apple Foundation Models) visualization page
        (Method::Get, "/fm") => routes::fm::view_fm(env).await,

        // 2026 page - key themes and links
        (Method::Get, "/2026") => routes::y2026::view_2026(env).await,

        // The Agent Network - Episode 200 transcript
        (Method::Get, "/the-agent-network") => routes::the_agent_network::view_the_agent_network(env).await,

        // Recursive Language Models - Episode 202 transcript
        (Method::Get, "/recursive-language-models") => routes::recursive_language_models::view_recursive_language_models(env).await,

        // Pylon and Nexus - Episode 203 transcript
        (Method::Get, "/pylon-and-nexus") => routes::pylon_and_nexus::view_pylon_and_nexus(env).await,

        // Install page
        (Method::Get, "/install") => routes::install::view_install(env).await,

        // Blog posts
        (Method::Get, "/blog/pylon-v0.1-release") => routes::pylon_v0_1_release::view_pylon_v0_1_release(env).await,
        (Method::Get, "/blog/nexus-v0.1-release") => routes::nexus_v0_1_release::view_nexus_v0_1_release(env).await,

        // OG image proxy - forward to og-worker
        (Method::Get, path) if path.starts_with("/og/") => {
            let og_url = format!("https://openagents-og.openagents.workers.dev{}", path);
            let og_req = Request::new(&og_url, Method::Get)?;
            Fetch::Request(og_req).send().await
        }

        // Homepage - shows landing/waitlist page
        (Method::Get, "/") => routes::early::view_early(env).await,

        // Early access page - old landing page
        (Method::Get, "/early") => routes::early::view_early(env).await,

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

        // Telemetry batch endpoint (sendBeacon)
        (Method::Post, "/api/telemetry/batch") => {
            let body = req.text().await?;
            routes::telemetry::handle_batch(env, body).await
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

        // AI completion routes (Stripe LLM proxy)
        (Method::Post, "/api/ai/chat") => {
            let body = req.text().await?;
            with_auth(&req, &env, |user| {
                routes::ai::chat_completion(user, env.clone(), body.clone())
            })
            .await
        }
        (Method::Post, "/api/ai/chat/stream") => {
            let body = req.text().await?;
            with_auth(&req, &env, |user| {
                routes::ai::chat_completion_stream(user, env.clone(), body.clone())
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

fn parse_agent_id(segment: &str) -> Result<i64> {
    segment
        .parse::<i64>()
        .map_err(|_| Error::RustError("Invalid agent id".to_string()))
}
