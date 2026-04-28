use axum::Json;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde::Serialize;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum RegistrarError {
    #[error("missing or invalid bearer token")]
    Unauthorized,
    #[error("invalid handle format")]
    InvalidHandle,
    #[error("handle is reserved")]
    ReservedHandle,
    #[error("handle already taken")]
    HandleTaken,
    #[error("invalid npub")]
    InvalidNpub,
    #[error("public key already registered to a different handle")]
    PubkeyTaken,
    #[error("handle not found")]
    NotFound,
    #[error("malformed request body")]
    BadRequest,
    #[error("challenge not found or already used")]
    ChallengeNotFound,
    #[error("challenge expired")]
    ChallengeExpired,
    #[error("challenge proof invalid: {0}")]
    ChallengeInvalid(&'static str),
    #[error("internal server error")]
    Internal,
}

impl RegistrarError {
    fn status(&self) -> StatusCode {
        match self {
            Self::Unauthorized => StatusCode::UNAUTHORIZED,
            Self::InvalidHandle
            | Self::ReservedHandle
            | Self::InvalidNpub
            | Self::BadRequest
            | Self::ChallengeInvalid(_) => StatusCode::BAD_REQUEST,
            Self::HandleTaken | Self::PubkeyTaken => StatusCode::CONFLICT,
            Self::NotFound | Self::ChallengeNotFound => StatusCode::NOT_FOUND,
            Self::ChallengeExpired => StatusCode::GONE,
            Self::Internal => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }

    fn code(&self) -> &'static str {
        match self {
            Self::Unauthorized => "unauthorized",
            Self::InvalidHandle => "invalid_handle",
            Self::ReservedHandle => "reserved_handle",
            Self::HandleTaken => "handle_taken",
            Self::InvalidNpub => "invalid_npub",
            Self::PubkeyTaken => "pubkey_taken",
            Self::NotFound => "not_found",
            Self::BadRequest => "bad_request",
            Self::ChallengeNotFound => "challenge_not_found",
            Self::ChallengeExpired => "challenge_expired",
            Self::ChallengeInvalid(_) => "challenge_invalid",
            Self::Internal => "internal_error",
        }
    }
}

#[derive(Serialize)]
struct ErrorBody {
    error: &'static str,
    message: String,
}

impl IntoResponse for RegistrarError {
    fn into_response(self) -> Response {
        let status = self.status();
        let body = Json(ErrorBody {
            error: self.code(),
            message: self.to_string(),
        });
        (status, body).into_response()
    }
}
