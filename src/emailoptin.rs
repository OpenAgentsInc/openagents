use axum::{
    extract::{Form, State},
    http::StatusCode,
    response::IntoResponse,
};
use sqlx::types::time::OffsetDateTime;
use sqlx::PgPool;
use uuid::Uuid;

#[derive(serde::Deserialize)]
pub struct FormData {
    email: String,
    name: String,
}

pub async fn subscribe(
    State(db): State<PgPool>,
    Form(form): Form<FormData>,
) -> impl IntoResponse {
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
    .execute(&db)
    .await
    {
        Ok(_) => StatusCode::OK.into_response(),
        Err(e) => {
            tracing::error!("Failed to execute query: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}