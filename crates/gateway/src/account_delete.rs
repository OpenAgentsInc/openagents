//! `DELETE /v1/account` (#12002): delete the signed-in account.
//!
//! App Store guideline 5.1.1(v) asks every app that makes an account to
//! let the person delete it from the app. The website's forwarder
//! (`openagents-web`, `account_delete`) deletes what the website keeps
//! first (chats, files, traces, connections, cloud computers), then calls
//! this route, which deletes what the account service keeps:
//!
//! - every bearer key the account holds (revoked),
//! - the provider keys saved in the workspaces it owns,
//! - its GitHub tokens, the projects it owns, and its place in projects
//!   shared with it (`oa_auth::repos::forget_account`),
//! - every session, app sign-in, and sign-in key, and its password,
//! - its name, linked identities, principals, and memberships
//!   (`tenancy::accounts::deletion`).
//!
//! Body: `{"confirm": "delete my account", "forfeit_balance"?: bool,
//! "check"?: bool}`. The call runs only under a session (`sess_` or a
//! sign-in key, never an `oak_` key), and only when `confirm` repeats
//! [`CONFIRM`]: deleting can't be undone, so it is never a single tap.
//! While a workspace the account owns still holds paid credit, the call
//! is refused (`409 balance_remaining`) unless `forfeit_balance` is
//! true. An owner of a team with other members is refused (`409
//! shared_workspace`) until someone else owns it. `check: true` runs
//! every check and deletes nothing, so the website can stop before it
//! deletes its own part.
//!
//! The wallet is not here: its 12 words stay on the person's device, and
//! its funds are not the account's to delete.

use std::sync::Arc;

use axum::Json;
use axum::extract::State;
use axum::extract::rejection::JsonRejection;
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use serde_json::{Value, json};
use tenancy::keys;
use tenancy::sessions;
use tenancy::workspaces::UserId;

use crate::accounts::{
    accounts_refusal, accounts_store, answered, member_account, principal, record, refused,
    sessions_refusal, sessions_store,
};
use crate::serve::ServeState;

/// The words the person types (or the app sends after they confirm) to
/// delete the account.
pub const CONFIRM: &str = "delete my account";

/// Whether `value` repeats [`CONFIRM`], ignoring case and outer spaces.
fn confirmed(value: Option<&str>) -> bool {
    value.is_some_and(|text| text.trim().eq_ignore_ascii_case(CONFIRM))
}

/// `DELETE /v1/account`.
pub(crate) async fn delete(
    State(state): State<Arc<ServeState>>,
    headers: HeaderMap,
    body: Result<Json<Value>, JsonRejection>,
) -> Response {
    let body = body.map_or(Value::Null, |Json(body)| body);
    let principal = match principal(&state, &headers) {
        Ok(principal) => principal,
        Err(response) => return response,
    };
    let account = match member_account(&principal) {
        Ok(account) => account.to_string(),
        Err(response) => return response,
    };
    if principal.session().is_none() {
        return refused(
            StatusCode::FORBIDDEN,
            "session_required",
            "Sign in to delete your account. An API key can't delete it.",
        );
    }
    if !confirmed(body["confirm"].as_str()) {
        return refused(
            StatusCode::BAD_REQUEST,
            "confirm_required",
            format!("To delete your account, confirm with \"{CONFIRM}\"."),
        );
    }
    let forfeit = body["forfeit_balance"].as_bool() == Some(true);
    let check = body["check"].as_bool() == Some(true);

    let accounts = match accounts_store(&state) {
        Ok(accounts) => accounts,
        Err(response) => return response,
    };
    let store = match accounts.store() {
        Ok(store) => store,
        Err(_) => return unavailable(),
    };
    let Some(holder) = store.accounts.get(&account) else {
        return refused(
            StatusCode::NOT_FOUND,
            "unknown_account",
            "No account matches your sign-in.",
        );
    };
    let principals = holder.principals.clone();
    let shared = match accounts.shared_owned(&account) {
        Ok(shared) => shared,
        Err(_) => return unavailable(),
    };
    if let Some(workspace) = shared.first() {
        let name = store
            .workspaces
            .get(workspace)
            .map_or_else(|| workspace.clone(), |ws| ws.name.clone());
        return refused(
            StatusCode::CONFLICT,
            "shared_workspace",
            format!(
                "You own the team \"{name}\", which has other members. Make one of them \
                 its owner, or remove them, and then delete your account."
            ),
        );
    }
    let owned: Vec<String> = store
        .workspaces
        .values()
        .filter(|ws| {
            ws.members.get(&account).is_some_and(|m| {
                m.status == tenancy::MemberStatus::Active && m.role == tenancy::Role::Owner
            })
        })
        .map(|ws| ws.id.clone())
        .collect();
    let paid = paid_credit(&state, &owned).await;
    if let Some((currency, amount)) = &paid
        && !forfeit
    {
        return balance_refused(currency, *amount);
    }
    if check {
        return answered(
            StatusCode::OK,
            json!({
                "account": {"id": account, "state": "active"},
                "ready": true,
                "balance": paid.as_ref().map(|(currency, amount)| json!({"currency": currency, "available": amount})),
            }),
        );
    }

    // Credentials first, so nothing can act for the account while the
    // rest goes.
    for principal_name in &principals {
        if let Some(key) = principal_name.strip_prefix("key:") {
            match keys::revoke(&state.dir, key) {
                Ok(()) | Err(keys::KeyTrouble::UnknownKey(_)) => {}
                Err(_) => return unavailable(),
            }
        }
    }
    let user = UserId::from(account.as_str());
    let actor = principal.actor().to_string();
    let session = principal.session().map(str::to_string);
    let sessions = match sessions_store(&state) {
        Ok(sessions) => sessions,
        Err(response) => return response,
    };
    if let Err(refusal) = sessions.mutate(|book, access, now| {
        let ended = book.revoke_all(&user, now);
        book.credentials.remove(&user);
        sessions::push_access(
            access,
            sessions::Access {
                at: now,
                actor: actor.clone(),
                action: "account-delete".to_string(),
                workspace: None,
                session: session.clone(),
                detail: Some(format!("{} sessions ended", ended.len())),
            },
        );
        Ok(())
    }) {
        return sessions_refusal(refusal);
    }
    if let Some(saved) = state.provider_keys.as_deref() {
        for workspace in &owned {
            if saved.forget_workspace(workspace).is_err() {
                return unavailable();
            }
        }
    }
    if oa_auth::repos::forget_account(&state.dir, &account).is_err() {
        return unavailable();
    }
    if let Err(refusal) = accounts.delete_account(&account) {
        return accounts_refusal(refusal);
    }
    record(&state, &principal, "account-delete", None, None);
    answered(
        StatusCode::OK,
        json!({"account": {"id": account, "state": "deleted"}}),
    )
}

/// The paid credit left in the workspaces `owned`, when there is any:
/// what is available, up to what was bought (a promotional credit is not
/// the person's money).
async fn paid_credit(state: &ServeState, owned: &[String]) -> Option<(String, u64)> {
    let ledger = state.money_lock().await?;
    let mut currency = None;
    let mut total = 0_u64;
    for workspace in owned {
        let Ok(balance) = ledger.balance(workspace) else {
            continue;
        };
        let paid = balance.available.min(balance.purchased_funding);
        if paid > 0 {
            total = total.saturating_add(paid);
            currency.get_or_insert(balance.currency);
        }
    }
    currency.map(|currency| (currency, total))
}

/// `409 balance_remaining`, with the amount beside the message for the
/// app to show.
fn balance_refused(currency: &str, amount: u64) -> Response {
    (
        StatusCode::CONFLICT,
        Json(json!({"error": {
            "code": "balance_remaining",
            "param": "forfeit_balance",
            "message": "You still have paid credit on your account. Deleting the account \
                        gives it up. Use it first, or confirm that you want to give it up.",
            "balance": {"currency": currency, "available": amount},
        }})),
    )
        .into_response()
}

fn unavailable() -> Response {
    refused(
        StatusCode::SERVICE_UNAVAILABLE,
        "accounts_unavailable",
        "We couldn't delete your account right now. Try again in a minute.",
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_phrase_confirms() {
        assert!(confirmed(Some("delete my account")));
        assert!(confirmed(Some("  Delete My Account ")));
        assert!(!confirmed(Some("delete")));
        assert!(!confirmed(Some("")));
        assert!(!confirmed(None));
    }
}
