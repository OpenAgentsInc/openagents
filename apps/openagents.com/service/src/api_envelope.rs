use std::collections::HashMap;

use axum::Json;
use axum::http::StatusCode;
use serde::Serialize;

pub type ApiErrorTuple = (StatusCode, Json<ApiErrorResponse>);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApiErrorCode {
    InvalidRequest,
    Unauthorized,
    Forbidden,
    RateLimited,
    NotFound,
    Conflict,
    InvalidScope,
    ServiceUnavailable,
    SyncTokenUnavailable,
    StaticAssetError,
    LegacyRouteUnavailable,
    InternalError,
}

impl ApiErrorCode {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::InvalidRequest => "invalid_request",
            Self::Unauthorized => "unauthorized",
            Self::Forbidden => "forbidden",
            Self::RateLimited => "rate_limited",
            Self::NotFound => "not_found",
            Self::Conflict => "conflict",
            Self::InvalidScope => "invalid_scope",
            Self::ServiceUnavailable => "service_unavailable",
            Self::SyncTokenUnavailable => "sync_token_unavailable",
            Self::StaticAssetError => "static_asset_error",
            Self::LegacyRouteUnavailable => "legacy_route_unavailable",
            Self::InternalError => "internal_error",
        }
    }

    pub const fn default_status(self) -> StatusCode {
        match self {
            Self::InvalidRequest => StatusCode::UNPROCESSABLE_ENTITY,
            Self::Unauthorized => StatusCode::UNAUTHORIZED,
            Self::Forbidden => StatusCode::FORBIDDEN,
            Self::RateLimited => StatusCode::TOO_MANY_REQUESTS,
            Self::NotFound => StatusCode::NOT_FOUND,
            Self::Conflict => StatusCode::CONFLICT,
            Self::InvalidScope => StatusCode::UNPROCESSABLE_ENTITY,
            Self::ServiceUnavailable => StatusCode::SERVICE_UNAVAILABLE,
            Self::SyncTokenUnavailable => StatusCode::SERVICE_UNAVAILABLE,
            Self::StaticAssetError => StatusCode::INTERNAL_SERVER_ERROR,
            Self::LegacyRouteUnavailable => StatusCode::SERVICE_UNAVAILABLE,
            Self::InternalError => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct ApiErrorDetail {
    pub code: &'static str,
    pub message: String,
}

#[derive(Debug, Serialize)]
pub struct ApiErrorResponse {
    pub message: String,
    pub error: ApiErrorDetail,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub errors: Option<HashMap<String, Vec<String>>>,
}

#[derive(Debug, Serialize)]
pub struct ApiDataEnvelope<T> {
    pub data: T,
}

pub fn ok_data<T: Serialize>(data: T) -> (StatusCode, Json<ApiDataEnvelope<T>>) {
    (StatusCode::OK, Json(ApiDataEnvelope { data }))
}

pub fn error_response(code: ApiErrorCode, message: impl Into<String>) -> ApiErrorTuple {
    error_response_with_status(code.default_status(), code, message)
}

pub fn error_response_with_status(
    status: StatusCode,
    code: ApiErrorCode,
    message: impl Into<String>,
) -> ApiErrorTuple {
    error_response_with_fields(status, code, message, None)
}

pub fn error_response_with_fields(
    status: StatusCode,
    code: ApiErrorCode,
    message: impl Into<String>,
    errors: Option<HashMap<String, Vec<String>>>,
) -> ApiErrorTuple {
    let message = message.into();
    (
        status,
        Json(ApiErrorResponse {
            message: message.clone(),
            error: ApiErrorDetail {
                code: code.as_str(),
                message,
            },
            errors,
        }),
    )
}

pub fn validation_error(field: &'static str, message: &str) -> ApiErrorTuple {
    let mut errors = HashMap::new();
    errors.insert(field.to_string(), vec![message.to_string()]);

    error_response_with_fields(
        StatusCode::UNPROCESSABLE_ENTITY,
        ApiErrorCode::InvalidRequest,
        message.to_string(),
        Some(errors),
    )
}

pub fn unauthorized_error(message: &str) -> ApiErrorTuple {
    error_response_with_status(
        StatusCode::UNAUTHORIZED,
        ApiErrorCode::Unauthorized,
        message.to_string(),
    )
}

pub fn forbidden_error(message: &str) -> ApiErrorTuple {
    error_response_with_status(
        StatusCode::FORBIDDEN,
        ApiErrorCode::Forbidden,
        message.to_string(),
    )
}

pub fn not_found_error(message: impl Into<String>) -> ApiErrorTuple {
    error_response_with_status(StatusCode::NOT_FOUND, ApiErrorCode::NotFound, message)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ApiErrorMatrixEntry {
    pub code: &'static str,
    pub status: u16,
    pub laravel_equivalent: &'static str,
}

const API_ERROR_MATRIX: [ApiErrorMatrixEntry; 12] = [
    ApiErrorMatrixEntry {
        code: "invalid_request",
        status: 422,
        laravel_equivalent: "validation_error",
    },
    ApiErrorMatrixEntry {
        code: "unauthorized",
        status: 401,
        laravel_equivalent: "unauthenticated",
    },
    ApiErrorMatrixEntry {
        code: "forbidden",
        status: 403,
        laravel_equivalent: "forbidden",
    },
    ApiErrorMatrixEntry {
        code: "rate_limited",
        status: 429,
        laravel_equivalent: "throttle_exceeded",
    },
    ApiErrorMatrixEntry {
        code: "not_found",
        status: 404,
        laravel_equivalent: "not_found",
    },
    ApiErrorMatrixEntry {
        code: "conflict",
        status: 409,
        laravel_equivalent: "conflict",
    },
    ApiErrorMatrixEntry {
        code: "invalid_scope",
        status: 422,
        laravel_equivalent: "invalid_scope",
    },
    ApiErrorMatrixEntry {
        code: "service_unavailable",
        status: 503,
        laravel_equivalent: "provider_unavailable",
    },
    ApiErrorMatrixEntry {
        code: "sync_token_unavailable",
        status: 503,
        laravel_equivalent: "sync_token_service_unavailable",
    },
    ApiErrorMatrixEntry {
        code: "static_asset_error",
        status: 500,
        laravel_equivalent: "asset_serve_failure",
    },
    ApiErrorMatrixEntry {
        code: "legacy_route_unavailable",
        status: 503,
        laravel_equivalent: "legacy_target_unavailable",
    },
    ApiErrorMatrixEntry {
        code: "internal_error",
        status: 500,
        laravel_equivalent: "internal_server_error",
    },
];

pub fn api_error_matrix() -> &'static [ApiErrorMatrixEntry] {
    &API_ERROR_MATRIX
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_matrix_codes_are_unique() {
        let mut codes = std::collections::HashSet::new();
        for row in api_error_matrix() {
            assert!(
                codes.insert(row.code),
                "duplicate error code in matrix: {}",
                row.code
            );
        }
    }

    #[test]
    fn validation_error_maps_to_expected_shape() {
        let (status, payload) = validation_error("code", "That code is invalid.");
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
        let body = serde_json::to_value(payload.0).expect("serialize payload");
        assert_eq!(body["error"]["code"], "invalid_request");
        assert_eq!(body["errors"]["code"][0], "That code is invalid.");
    }

    #[test]
    fn ok_data_wraps_payload_in_data_envelope() {
        let (_status, payload) = ok_data(serde_json::json!({"ok": true}));
        let body = serde_json::to_value(payload.0).expect("serialize payload");
        assert_eq!(body["data"]["ok"], true);
    }
}
