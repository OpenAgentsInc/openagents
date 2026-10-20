//! Account settings for the shared forms. Cookie requests pass through
//! cookie_api; bearer requests use the same account authentication.
use super::*;
use crate::coder_sync::{answer, refused as reject};
use axum::Json;
use serde_json::{Value, json};

pub(super) fn routes() -> Router<App> {
    Router::new()
        .route("/v1/settings/claude", get(status).put(save).delete(remove))
        .route(
            "/v1/settings/data/chats",
            axum::routing::delete(delete_chats),
        )
}

async fn viewer(app: &App, headers: &HeaderMap) -> Result<Viewer, Response> {
    let token = crate::account_export::app_token(headers).ok_or_else(|| {
        reject(
            StatusCode::UNAUTHORIZED,
            "signed_out",
            "Sign in to change your settings.",
        )
    })?;
    // Cookie requests keep the workspace selected on the HTML Settings page.
    // Explicit bearer requests use the account's own workspace.
    if crate::chat_owner::session(headers).as_deref() == Some(token) {
        let service = app.config.cloud.as_deref().ok_or_else(unavailable)?;
        return service.authenticate(headers).await.map_err(|_| {
            reject(
                StatusCode::UNAUTHORIZED,
                "signed_out",
                "Sign in to change your settings.",
            )
        });
    }
    crate::account_export::app_viewer(app, token)
        .await
        .map(|(_, viewer)| viewer)
}

fn unavailable() -> Response {
    reject(
        StatusCode::SERVICE_UNAVAILABLE,
        "unavailable",
        "We couldn't read your settings. Try again.",
    )
}

fn state(app: &App, viewer: &Viewer) -> Result<Value, Response> {
    let Some(computers) = app.config.cloud_byo.as_deref() else {
        return Ok(json!({"available":false}));
    };
    let owner = Owner::from_viewer(viewer).map_err(|_| {
        reject(
            StatusCode::FORBIDDEN,
            "workspace_required",
            "Choose a workspace to save a key.",
        )
    })?;
    let saved = computers.status(&owner, now()).map_err(|_| unavailable())?;
    Ok(json!({
        "available":true,
        "subscription_allowed":app.config.claude_tokens.admits_viewer(viewer),
        "saved":saved.map(|s| json!({"material":s.material,"label":material_label(s.material),"fingerprint":s.masked()}))
    }))
}

async fn status(State(app): State<App>, headers: HeaderMap) -> Response {
    let viewer = match viewer(&app, &headers).await {
        Ok(v) => v,
        Err(r) => return r,
    };
    match state(&app, &viewer) {
        Ok(s) => answer(StatusCode::OK, s),
        Err(r) => r,
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Credential {
    material: Material,
    value: String,
    consent: bool,
}

fn checked_material(form: &Credential, subscription_allowed: bool) -> Result<Material, Response> {
    let material = byo::detect(form.material, &form.value);
    if material == Material::OpenAiApiKey {
        return Err(reject(
            StatusCode::BAD_REQUEST,
            "invalid",
            "Choose a Claude provider.",
        ));
    }
    if material == Material::ClaudeSubscriptionToken && !subscription_allowed {
        return Err(reject(StatusCode::FORBIDDEN, "not_allowed", TOKEN_REFUSED));
    }
    if !form.consent {
        return Err(reject(
            StatusCode::BAD_REQUEST,
            "consent_required",
            "Check the box to save the key.",
        ));
    }
    Ok(material)
}

async fn save(
    State(app): State<App>,
    headers: HeaderMap,
    Json(mut form): Json<Credential>,
) -> Response {
    let viewer = match viewer(&app, &headers).await {
        Ok(v) => v,
        Err(r) => return r,
    };
    let material = match checked_material(&form, app.config.claude_tokens.admits_viewer(&viewer)) {
        Ok(m) => m,
        Err(r) => return r,
    };
    let (Some(computers), Ok(owner)) =
        (app.config.cloud_byo.as_deref(), Owner::from_viewer(&viewer))
    else {
        return unavailable();
    };
    let key = match Key::for_material(material, std::mem::take(&mut form.value)) {
        Ok(key) => key,
        Err(_) => {
            return reject(
                StatusCode::BAD_REQUEST,
                "invalid",
                "That isn't a valid credential for this provider. Nothing was saved.",
            );
        }
    };
    if let Err(error) = computers.check(material, &key).await {
        return match error {
            byo::CheckError::Refused => reject(
                StatusCode::BAD_REQUEST,
                "refused",
                "The provider didn't accept that credential. Nothing was saved.",
            ),
            byo::CheckError::Unreachable => reject(
                StatusCode::BAD_GATEWAY,
                "unavailable",
                "The provider couldn't be reached. Nothing was saved. Try again.",
            ),
        };
    }
    if computers.store(&owner, material, key, true, now()).is_err() {
        return unavailable();
    }
    match state(&app, &viewer) {
        Ok(s) => answer(StatusCode::OK, s),
        Err(r) => r,
    }
}

async fn remove(State(app): State<App>, headers: HeaderMap) -> Response {
    let viewer = match viewer(&app, &headers).await {
        Ok(v) => v,
        Err(r) => return r,
    };
    let (Some(computers), Ok(owner)) =
        (app.config.cloud_byo.as_deref(), Owner::from_viewer(&viewer))
    else {
        return unavailable();
    };
    if computers.revoke(&owner).is_err() {
        return unavailable();
    }
    match state(&app, &viewer) {
        Ok(s) => answer(StatusCode::OK, s),
        Err(r) => r,
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DeleteChats {
    confirm: String,
}

async fn delete_chats(
    State(app): State<App>,
    headers: HeaderMap,
    Json(form): Json<DeleteChats>,
) -> Response {
    let viewer = match viewer(&app, &headers).await {
        Ok(v) => v,
        Err(r) => return r,
    };
    if form.confirm != "delete all chats" {
        return reject(
            StatusCode::BAD_REQUEST,
            "confirm_required",
            "Confirm that you want to delete all your chats.",
        );
    }
    let owner = crate::chat_store::account_owner(&viewer.account_id);
    match crate::pages::chat::delete_all::delete_all(&app.config.chat_store, &owner).await {
        Ok(left) => answer(StatusCode::OK, json!({"remaining":left})),
        Err(error) => crate::coder_sync::stored(&error),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::{Body, to_bytes};
    use axum::http::Request;
    use tower::ServiceExt;

    #[test]
    fn consent_and_subscription_allowlist_are_checked_before_using_the_value() {
        let mut form = Credential {
            material: Material::AnthropicApiKey,
            value: "sk-ant-oat01-private-test-value".into(),
            consent: true,
        };
        assert_eq!(
            checked_material(&form, false).unwrap_err().status(),
            StatusCode::FORBIDDEN
        );
        assert_eq!(
            checked_material(&form, true).unwrap(),
            Material::ClaudeSubscriptionToken
        );
        form.consent = false;
        assert_eq!(
            checked_material(&form, true).unwrap_err().status(),
            StatusCode::BAD_REQUEST
        );
        form.material = Material::OpenAiApiKey;
        form.consent = true;
        assert_eq!(
            checked_material(&form, true).unwrap_err().status(),
            StatusCode::BAD_REQUEST
        );
    }

    #[tokio::test]
    async fn settings_api_requires_account_auth_and_never_echoes_a_credential() {
        let root = tempfile::tempdir().unwrap();
        let app = crate::router(crate::Config::development(root.path().into()));
        for (method, path, body) in [
            ("GET", "/v1/settings/claude", ""),
            (
                "PUT",
                "/v1/settings/claude",
                r#"{"material":"anthropic_api_key","value":"private-test-key","consent":true}"#,
            ),
            ("DELETE", "/v1/settings/claude", ""),
            (
                "DELETE",
                "/v1/settings/data/chats",
                r#"{"confirm":"delete all chats"}"#,
            ),
            ("GET", "/v1/account/export", ""),
        ] {
            let response = app
                .clone()
                .oneshot(
                    Request::builder()
                        .method(method)
                        .uri(path)
                        .header(header::HOST, "127.0.0.1:4300")
                        .header(header::CONTENT_TYPE, "application/json")
                        .body(Body::from(body))
                        .unwrap(),
                )
                .await
                .unwrap();
            assert_eq!(
                response.status(),
                if path == "/v1/account/export" {
                    StatusCode::SERVICE_UNAVAILABLE
                } else {
                    StatusCode::UNAUTHORIZED
                },
                "{method} {path}"
            );
            let body = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
            assert!(!String::from_utf8_lossy(&body).contains("private-test-key"));
        }
    }

    #[tokio::test]
    async fn cross_site_cookie_cannot_change_settings_or_download_account_data() {
        let root = tempfile::tempdir().unwrap();
        let app = crate::router(crate::Config::development(root.path().into()));
        for path in [
            "/v1/settings/claude",
            "/v1/settings/data/chats",
            "/v1/account/export",
        ] {
            let response = app
                .clone()
                .oneshot(
                    Request::builder()
                        .uri(path)
                        .header(header::HOST, "127.0.0.1:4300")
                        .header(header::COOKIE, "oa_cloud_session=sess_test")
                        .header("sec-fetch-site", "cross-site")
                        .body(Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::FORBIDDEN, "{path}");
        }
    }
}
