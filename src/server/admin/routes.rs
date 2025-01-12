use crate::nostr::event::Event;
use crate::{configuration, database};
use actix_web::{cookie::Cookie, get, post, web, HttpResponse, Responder, Result};
use secp256k1::{rand, KeyPair, Message, Secp256k1};
use serde::Deserialize;
use serde_json::json;

#[get("/stats")]
pub async fn admin_stats() -> Result<HttpResponse> {
    let config = configuration::get_configuration()
        .map_err(|e| actix_web::error::ErrorInternalServerError(format!("Config error: {}", e)))?;

    let pool = match database::get_connection_pool(&config).await {
        Ok(pool) => pool,
        Err(e) => {
            return Ok(HttpResponse::InternalServerError().json(json!({
                "error": format!("Database error: {}", e)
            })))
        }
    };

    // Get total events count
    let total_events: i64 = match sqlx::query_scalar("SELECT COUNT(*) FROM events")
        .fetch_one(&pool)
        .await
    {
        Ok(count) => count,
        Err(e) => {
            return Ok(HttpResponse::InternalServerError().json(json!({
                "error": format!("Failed to get event count: {}", e)
            })))
        }
    };

    // Get events by kind
    let kinds: Vec<(i32, i64)> = match sqlx::query_as(
        "SELECT kind, COUNT(*) as count 
         FROM events 
         GROUP BY kind 
         ORDER BY kind",
    )
    .fetch_all(&pool)
    .await
    {
        Ok(kinds) => kinds,
        Err(e) => {
            return Ok(HttpResponse::InternalServerError().json(json!({
                "error": format!("Failed to get event kinds: {}", e)
            })))
        }
    };

    let events_by_kind: serde_json::Map<String, serde_json::Value> = kinds
        .into_iter()
        .map(|(kind, count)| (kind.to_string(), json!(count)))
        .collect();

    // Get database size
    let db_size: i64 = match sqlx::query_scalar("SELECT pg_database_size(current_database())")
        .fetch_one(&pool)
        .await
    {
        Ok(size) => size,
        Err(e) => {
            return Ok(HttpResponse::InternalServerError().json(json!({
                "error": format!("Failed to get database size: {}", e)
            })))
        }
    };

    Ok(HttpResponse::Ok().json(json!({
        "total_events": total_events,
        "events_by_kind": events_by_kind,
        "storage_usage": format!("{:.1} MB", db_size as f64 / (1024.0 * 1024.0)),
        "index_usage": [],
        "status": "ok"
    })))
}

#[derive(Deserialize)]
pub struct LoginForm {
    password: String,
}

#[get("")]
pub async fn admin_dashboard() -> impl Responder {
    HttpResponse::Ok()
        .content_type("text/html")
        .body(include_str!("../../../templates/admin/dashboard.html"))
}

#[get("/login")]
pub async fn admin_login() -> impl Responder {
    HttpResponse::Ok()
        .content_type("text/html")
        .body(include_str!("../../../templates/admin/login.html"))
}

#[post("/login")]
pub async fn admin_login_post(form: web::Form<LoginForm>) -> impl Responder {
    let config = match configuration::get_configuration() {
        Ok(config) => config,
        Err(_) => return HttpResponse::InternalServerError().finish(),
    };

    if form.password == config.application.admin_token {
        HttpResponse::Found()
            .cookie(
                Cookie::build("admin_session", config.application.admin_token)
                    .path("/admin")
                    .secure(true)
                    .http_only(true)
                    .finish(),
            )
            .append_header(("Location", "/admin"))
            .finish()
    } else {
        HttpResponse::Found()
            .append_header(("Location", "/admin/login?error=1"))
            .finish()
    }
}

use bitcoin_hashes::{sha256, Hash};

#[post("/demo-event")]
pub async fn create_demo_event() -> Result<HttpResponse> {
    let secp = Secp256k1::new();
    let keypair = KeyPair::new(&secp, &mut rand::thread_rng());

    let mut event = Event {
        id: "".to_string(),
        pubkey: keypair
            .public_key()
            .x_only_public_key()
            .0
            .serialize()
            .iter()
            .fold(String::with_capacity(64), |mut acc, b| {
                use std::fmt::Write;
                write!(acc, "{:02x}", b).unwrap();
                acc
            }),
        created_at: chrono::Utc::now().timestamp(),
        kind: 1,
        tags: vec![vec!["t".to_string(), "demo".to_string()]],
        content: "This is a demo event".to_string(),
        sig: "".to_string(),
        tagidx: None,
    };

    // Generate event ID
    if let Some(canonical) = event.to_canonical() {
        let digest: sha256::Hash = sha256::Hash::hash(canonical.as_bytes());
        event.id = format!("{:x}", digest);

        // Sign the event
        let msg = Message::from_slice(digest.as_ref()).expect("32 bytes");
        let sig = secp.sign_schnorr_with_rng(&msg, &keypair, &mut rand::thread_rng());
        event.sig = sig.to_string();

        // Validate the event
        if let Err(e) = event.validate() {
            return Ok(HttpResponse::InternalServerError().json(json!({
                "status": "error",
                "message": format!("Event validation failed: {}", e)
            })));
        }

        let config = configuration::get_configuration().map_err(|e| {
            actix_web::error::ErrorInternalServerError(format!("Config error: {}", e))
        })?;

        let pool = match database::get_connection_pool(&config).await {
            Ok(pool) => pool,
            Err(e) => {
                return Ok(HttpResponse::InternalServerError().json(json!({
                    "status": "error",
                    "message": format!("Database error: {}", e)
                })))
            }
        };

        if let Err(e) = sqlx::query(
            "INSERT INTO events (id, pubkey, created_at, kind, tags, content, sig) 
             VALUES ($1, $2, $3, $4, $5, $6, $7)",
        )
        .bind(&event.id)
        .bind(&event.pubkey)
        .bind(event.created_at)
        .bind(event.kind)
        .bind(serde_json::to_value(&event.tags).unwrap())
        .bind(&event.content)
        .bind(&event.sig)
        .execute(&pool)
        .await
        {
            return Ok(HttpResponse::InternalServerError().json(json!({
                "status": "error",
                "message": format!("Failed to save event: {}", e)
            })));
        }

        Ok(HttpResponse::Ok().json(json!({
            "status": "success",
            "event": event
        })))
    } else {
        Ok(HttpResponse::InternalServerError().json(json!({
            "status": "error",
            "message": "Failed to canonicalize event"
        })))
    }
}

pub fn admin_config(cfg: &mut web::ServiceConfig) {
    cfg.service(admin_dashboard)
        .service(admin_login)
        .service(admin_login_post)
        .service(admin_stats)
        .service(create_demo_event);
}
