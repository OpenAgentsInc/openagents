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

use bitcoin_hashes::{sha256, Hash};
use secp256k1::Message;

#[post("/demo-event")]
pub async fn create_demo_event() -> impl Responder {
    let secp = Secp256k1::new();
    let keypair = KeyPair::new(&secp, &mut rand::thread_rng());
    
    let mut event = Event {
        id: "".to_string(),
        pubkey: keypair.public_key().x_only_public_key().0.serialize().iter()
            .map(|b| format!("{:02x}", b))
            .collect::<String>(),
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
            return HttpResponse::InternalServerError().json(json!({
                "status": "error",
                "message": format!("Event validation failed: {}", e)
            }));
        }

        // TODO: Save to database
        
        HttpResponse::Ok().json(json!({
            "status": "success",
            "event": event
        }))
    } else {
        HttpResponse::InternalServerError().json(json!({
            "status": "error",
            "message": "Failed to canonicalize event"
        }))
    }
}

pub fn admin_config(cfg: &mut web::ServiceConfig) {
    cfg.service(admin_dashboard)
       .service(admin_login)
       .service(admin_login_post)
       .service(admin_stats)
       .service(create_demo_event);
}

#[cfg(test)]
mod tests {
    use super::*;
    use actix_web::{test, App};

    #[actix_web::test]
    async fn test_create_demo_event() {
        let app = test::init_service(
            App::new().service(create_demo_event)
        ).await;

        let req = test::TestRequest::post()
            .uri("/demo-event")
            .to_request();

        let resp: serde_json::Value = test::call_and_read_body_json(&app, req).await;
        
        assert_eq!(resp["status"], "success");
        
        let event = &resp["event"];
        assert!(!event["id"].as_str().unwrap().is_empty());
        assert!(!event["pubkey"].as_str().unwrap().is_empty());
        assert!(!event["sig"].as_str().unwrap().is_empty());
        assert_eq!(event["kind"], 1);
        assert_eq!(event["content"], "This is a demo event");
        assert_eq!(event["tags"][0][0], "t");
        assert_eq!(event["tags"][0][1], "demo");
    }

    #[actix_web::test]
    async fn test_admin_stats() {
        let app = test::init_service(
            App::new().service(admin_stats)
        ).await;

        let req = test::TestRequest::get()
            .uri("/stats")
            .to_request();

        let resp: serde_json::Value = test::call_and_read_body_json(&app, req).await;
        
        assert!(resp.get("total_events").is_some());
        assert!(resp.get("events_by_kind").is_some());
        assert!(resp.get("storage_usage").is_some());
    }
}
