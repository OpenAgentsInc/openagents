// OpenAgents Web Platform
// Provides GitHub OAuth, Stripe checkout, and autopilot execution as a service

use actix_web::{web, App, HttpResponse, HttpServer, Result};
use serde::{Deserialize, Serialize};
use tracing::{info, warn};

mod github;
mod stripe_integration;
mod autopilot_runner;
mod db;

#[derive(Debug, Serialize)]
struct HealthResponse {
    status: String,
    version: String,
}

async fn health() -> Result<HttpResponse> {
    Ok(HttpResponse::Ok().json(HealthResponse {
        status: "healthy".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
    }))
}

async fn index() -> Result<HttpResponse> {
    Ok(HttpResponse::Ok().content_type("text/html").body(INDEX_HTML))
}

const INDEX_HTML: &str = r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>OpenAgents - Autonomous Code Execution</title>
    <style>
        * { margin: 0; padding: 0; box-sizing: border-box; }
        body {
            font-family: 'Vera Mono', 'Monaco', 'Courier New', monospace;
            background: #0a0a0a;
            color: #e0e0e0;
            line-height: 1.6;
        }
        .container {
            max-width: 900px;
            margin: 0 auto;
            padding: 40px 20px;
        }
        h1 {
            font-size: 2.5rem;
            margin-bottom: 1rem;
            color: #00ff41;
        }
        .tagline {
            font-size: 1.2rem;
            margin-bottom: 3rem;
            color: #888;
        }
        .cta-button {
            display: inline-block;
            background: #00ff41;
            color: #0a0a0a;
            padding: 15px 30px;
            text-decoration: none;
            font-weight: bold;
            border: none;
            cursor: pointer;
            margin-right: 15px;
            transition: background 0.2s;
        }
        .cta-button:hover {
            background: #00cc33;
        }
        .secondary-button {
            background: transparent;
            color: #00ff41;
            border: 2px solid #00ff41;
        }
        .secondary-button:hover {
            background: #00ff41;
            color: #0a0a0a;
        }
        .features {
            margin-top: 4rem;
            display: grid;
            grid-template-columns: repeat(auto-fit, minmax(250px, 1fr));
            gap: 2rem;
        }
        .feature {
            padding: 1.5rem;
            border: 1px solid #333;
            background: #111;
        }
        .feature h3 {
            color: #00ff41;
            margin-bottom: 0.5rem;
        }
        .stats {
            margin-top: 3rem;
            display: flex;
            gap: 3rem;
        }
        .stat {
            text-align: center;
        }
        .stat-number {
            font-size: 2rem;
            color: #00ff41;
            font-weight: bold;
        }
        .stat-label {
            color: #888;
            font-size: 0.9rem;
        }
    </style>
</head>
<body>
    <div class="container">
        <h1>OpenAgents</h1>
        <p class="tagline">Connect a repo and a credit card. Go do something else. We'll take it from here.</p>

        <div>
            <a href="/auth/github" class="cta-button">Connect GitHub</a>
            <a href="/demo" class="cta-button secondary-button">View Demo</a>
        </div>

        <div class="features">
            <div class="feature">
                <h3>Autonomous Execution</h3>
                <p>AI agents that write code, run tests, and create PRs while you sleep. 4x faster than copilot mode.</p>
            </div>
            <div class="feature">
                <h3>Pay As You Go</h3>
                <p>Credit-based billing. Only pay for what you use. Bitcoin Lightning and Stripe supported.</p>
            </div>
            <div class="feature">
                <h3>Full Audit Trail</h3>
                <p>Every decision, every file read, every test run logged. Complete transparency.</p>
            </div>
        </div>

        <div class="stats">
            <div class="stat">
                <div class="stat-number">1,500+</div>
                <div class="stat-label">Tests Passing</div>
            </div>
            <div class="stat">
                <div class="stat-number">86</div>
                <div class="stat-label">NIPs Implemented</div>
            </div>
            <div class="stat">
                <div class="stat-number">19</div>
                <div class="stat-label">Actions/Minute</div>
            </div>
        </div>
    </div>
</body>
</html>
"#;

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    // Load environment variables
    dotenvy::dotenv().ok();

    let port = std::env::var("PORT")
        .unwrap_or_else(|_| "8080".to_string())
        .parse::<u16>()
        .expect("PORT must be a valid u16");

    info!("Starting OpenAgents Web Platform on port {}", port);

    // Initialize database
    let db_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "sqlite:web-platform.db".to_string());

    info!("Connecting to database: {}", db_url);

    HttpServer::new(move || {
        App::new()
            .route("/", web::get().to(index))
            .route("/health", web::get().to(health))
            // GitHub OAuth routes
            .service(
                web::scope("/auth")
                    .route("/github", web::get().to(github::start_oauth))
                    .route("/github/callback", web::get().to(github::oauth_callback))
            )
            // Stripe checkout routes
            .service(
                web::scope("/checkout")
                    .route("/create-session", web::post().to(stripe_integration::create_checkout_session))
                    .route("/success", web::get().to(stripe_integration::checkout_success))
                    .route("/cancel", web::get().to(stripe_integration::checkout_cancel))
                    .route("/webhook", web::post().to(stripe_integration::stripe_webhook))
            )
            // Autopilot routes
            .service(
                web::scope("/autopilot")
                    .route("/start", web::post().to(autopilot_runner::start_job))
                    .route("/status/{job_id}", web::get().to(autopilot_runner::job_status))
                    .route("/cancel/{job_id}", web::post().to(autopilot_runner::cancel_job))
            )
    })
    .bind(("0.0.0.0", port))?
    .run()
    .await
}
