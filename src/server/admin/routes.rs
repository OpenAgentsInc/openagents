use actix_web::{get, post, web, HttpResponse, Responder, cookie::Cookie};
use serde_json::json;
use serde::Deserialize;

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

pub fn admin_config(cfg: &mut web::ServiceConfig) {
    cfg.service(admin_dashboard)
       .service(admin_login)
       .service(admin_login_post)
       .service(admin_stats);
}
