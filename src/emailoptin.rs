#[allow(unused_imports)]
use actix_web::{web, HttpResponse};
#[allow(unused_imports)]
use sqlx::types::time::OffsetDateTime;
use sqlx::PgPool;
#[allow(unused_imports)]
use uuid::Uuid;

#[derive(serde::Deserialize)]
pub struct FormData {
    email: String,
    name: String,
}

pub async fn subscribe(form: web::Form<FormData>, db: web::Data<PgPool>) -> HttpResponse {
    match sqlx::query!(
        r#"
        INSERT INTO subscriptions (id, email, name, subscribed_at)
        VALUES ($1, $2, $3, $4)
        "#,
        Uuid::new_v4(),
        form.email,
        form.name,
        OffsetDateTime::now_utc()
    )
    .execute(db.as_ref())
    .await
    {
        Ok(_) => HttpResponse::Ok().finish(),
        Err(e) => {
            tracing::error!("Failed to execute query: {}", e);
            HttpResponse::InternalServerError().finish()
        }
    }
}