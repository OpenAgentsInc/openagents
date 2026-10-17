//! `DELETE /v1/account` (#12002): the apps' Delete account.
//!
//! The phone and desktop apps call this with their own token
//! (`Authorization: Bearer sess_…` or a sign-in key) and a body of
//! `{"confirm": "delete my account", "forfeit_balance"?: bool}`. The
//! website deletes its own part, then the account service's
//! (`gateway::account_delete`):
//!
//! 1. The account service checks the confirmation, the session, a team
//!    that still has other members, and paid credit, and deletes nothing
//!    (`check: true`). A refusal is passed on as it came, so the app can
//!    show it; `400 confirm_required` also carries `export_url`, the
//!    account export ([`crate::account_export`]) to offer first.
//! 2. This server deletes what it keeps for the account: its cloud
//!    computers (through oa-computers), its connections (Google), its
//!    saved Claude credential, its vault, and everything in the chat
//!    store under the account's folder (chats, files, traces, project
//!    files, computer records).
//! 3. The account service deletes the rest: keys, sessions, GitHub
//!    tokens, projects, and the account's name and identities.
//!
//! A failure in step 2 leaves the account signed in, so the app can try
//! again. The wallet is not touched: its 12 words stay on the device.

use axum::Router;
use axum::body::Bytes;
use axum::extract::State;
use axum::http::{HeaderMap, Method, StatusCode};
use axum::response::Response;
use axum::routing::delete;
use serde_json::{Value, json};

use crate::App;
use crate::chat_store::account_owner;
use crate::cloud::session::Viewer;
use crate::coder_sync::{answer, refused};

/// The route.
pub(crate) const PATH: &str = "/v1/account";

pub(crate) fn routes() -> Router<App> {
    Router::new().route(PATH, delete(remove))
}

/// Whether this server answers `method path` itself. Only the delete:
/// `GET /v1/account` still goes to the account service.
pub(crate) fn owns(method: &Method, path: &str) -> bool {
    method == Method::DELETE && path == PATH
}

async fn remove(State(app): State<App>, headers: HeaderMap, body: Bytes) -> Response {
    let Some(token) = crate::account_export::app_token(&headers) else {
        return refused(
            StatusCode::UNAUTHORIZED,
            "signed_out",
            "Sign in to delete your account.",
        );
    };
    let (service, viewer) = match crate::account_export::app_viewer(&app, token).await {
        Ok(found) => found,
        Err(response) => return response,
    };
    let sent: Value = serde_json::from_slice(&body).unwrap_or(Value::Null);
    let request = json!({
        "confirm": sent["confirm"],
        "forfeit_balance": sent["forfeit_balance"].as_bool() == Some(true),
    });
    let export_url = format!(
        "{}{}",
        service.origin().trim_end_matches('/'),
        crate::account_export::PATH
    );
    let mut checking = request.clone();
    checking["check"] = json!(true);
    match service.delete_account(token, checking).await {
        Ok(found) if found.status == 200 => {}
        Ok(found) => return passed_on(found.status, found.body, &export_url),
        Err(_) => return unavailable(),
    }
    if let Err(part) = forget(&app, &viewer).await {
        eprintln!("openagents-web: account delete: {part} wasn't deleted");
        return refused(
            StatusCode::SERVICE_UNAVAILABLE,
            "unavailable",
            "We couldn't delete everything yet. Try again in a minute.",
        );
    }
    match service.delete_account(token, request).await {
        Ok(found) => passed_on(found.status, found.body, &export_url),
        Err(_) => unavailable(),
    }
}

/// The account service's answer as this server's.
fn passed_on(status: u16, mut body: Value, export_url: &str) -> Response {
    let status = StatusCode::from_u16(status).unwrap_or(StatusCode::BAD_GATEWAY);
    if body["error"]["code"] == "confirm_required" {
        body["error"]["export_url"] = json!(export_url);
    }
    answer(status, body)
}

fn unavailable() -> Response {
    refused(
        StatusCode::SERVICE_UNAVAILABLE,
        "unavailable",
        "We couldn't delete your account right now. Try again in a minute.",
    )
}

/// Delete what this server keeps for `viewer`'s account. The error names
/// the part that failed, for the log.
async fn forget(app: &App, viewer: &Viewer) -> Result<(), &'static str> {
    let owner = account_owner(&viewer.account_id);
    if let Some(client) = crate::computers::client() {
        let computers = crate::computers::list(client, &owner)
            .await
            .map_err(|_| "cloud computers")?;
        for computer in computers {
            let path = format!("/v1/computers/{}", computer.id);
            match client
                .call(reqwest::Method::DELETE, &path, &owner, None)
                .await
            {
                Ok((200..=299 | 404, _)) => {}
                _ => return Err("a cloud computer"),
            }
        }
    }
    if let Some(connections) = app.config.connections.as_deref() {
        connections
            .update(&owner, |account| {
                *account = crate::cloud::connections::Account::default();
            })
            .map_err(|_| "connections")?;
    }
    if let Some(byo) = app.config.cloud_byo.as_deref()
        && let Ok(byo_owner) = crate::cloud::byo::Owner::from_viewer(viewer)
    {
        byo.revoke(&byo_owner)
            .map_err(|_| "the Claude credential")?;
    }
    if let Some(vault) = app.config.vault_store.as_deref() {
        vault.remove_owner(&owner).await.map_err(|_| "the vault")?;
    }
    app.config
        .chat_store
        .remove_owner(&owner)
        .await
        .map_err(|_| "chats")?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_delete_is_answered_here() {
        assert!(owns(&Method::DELETE, "/v1/account"));
        assert!(!owns(&Method::GET, "/v1/account"));
        assert!(!owns(&Method::DELETE, "/v1/account/sessions/x"));
    }

    #[test]
    fn a_missing_confirmation_offers_the_export() {
        let body = json!({"error": {"code": "confirm_required", "message": "Confirm."}});
        let response = passed_on(400, body, "https://openagents.com/settings/export");
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[test]
    fn what_people_read_is_plain() {
        for text in [
            "Sign in to delete your account.",
            "We couldn't delete everything yet. Try again in a minute.",
            "We couldn't delete your account right now. Try again in a minute.",
        ] {
            assert!(oa_copy::violations(text, &[]).is_empty(), "{text}");
        }
    }
}
