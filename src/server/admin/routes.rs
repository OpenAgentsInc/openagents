use actix_web::{get, post, web, HttpResponse, Responder, cookie::Cookie};
use serde_json::json;
use serde::Deserialize;
use secp256k1::{rand, KeyPair, Secp256k1};
use crate::event::Event;

#[get("/stats")]
pub async fn admin_stats() -> impl Responder {
    // TODO: Implement actual database stats
    HttpResponse::Ok().json(json!({
        "total_events": 0,
        "events_by_kind": {},
        "storage_usage": "0 MB",
        "index_usage": []
    }))
}

#[derive(Deserialize)]
pub struct LoginForm {
    password: String,
}

#[get("")]
pub async fn admin_dashboard() -> impl Responder {
    HttpResponse::Ok().content_type("text/html").body(include_str!("../../../templates/admin/dashboard.html"))
}

#[get("/login")]
pub async fn admin_login() -> impl Responder {
    HttpResponse::Ok().content_type("text/html").body(include_str!("../../../templates/admin/login.html"))
}

#[post("/login")]
pub async fn admin_login_post(form: web::Form<LoginForm>) -> impl Responder {
    let config = match crate::configuration::get_configuration() {
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

#[post("/demo-event")]
pub async fn create_demo_event() -> impl Responder {
    let secp = Secp256k1::new();
    let keypair = KeyPair::new(&secp, &mut rand::thread_rng());
    
    let event = Event {
        id: "".to_string(), // Will be computed during validation
        pubkey: keypair.public_key().x_only_public_key().0.to_string(),
        created_at: chrono::Utc::now().timestamp(),
        kind: 1,
        tags: vec![vec!["t".to_string(), "demo".to_string()]],
        content: "This is a demo event".to_string(),
        sig: "".to_string(), // Will be computed during validation
        tagidx: None,
    };

    // TODO: Save to database
    
    HttpResponse::Ok().json(json!({
        "status": "success",
        "event": event
    }))
}

pub fn admin_config(cfg: &mut web::ServiceConfig) {
    cfg.service(admin_dashboard)
       .service(admin_login)
       .service(admin_login_post)
       .service(admin_stats)
       .service(create_demo_event);
}
