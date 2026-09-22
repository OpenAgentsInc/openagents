//! Opt-in product updates — verified subscriptions only.
//!
//! `PUT /v1/updates` records the caller's subscription: which topics
//! they consented to and an optional contact. `GET /v1/updates` reads it
//! back. `DELETE /v1/updates` unsubscribes — the record stays on disk as
//! the audit trail that the opt-out happened.
//!
//! A subscription exists only under an authenticated credential — that
//! authentication is the verification: no record exists for an address
//! or caller that did not prove control of a key. Product and support
//! consent are separate booleans; a sender must not treat one as the
//! other. Marketing mail to a `product: false` or `unsubscribed` record
//! is a policy violation, never a silent upgrade.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Json, Response};
use serde_json::{Value, json};

use crate::serve::{ServeState, authenticate, now_utc};

/// The subscription body schema tag.
const V: &str = "openagents.updates-subscription.v1";

const MAX_CONTACT: usize = 256;

/// `PUT /v1/updates`: create or replace the caller's subscription. The
/// whole record is idempotent — repeating the same body returns the same
/// state, and preferences change only through this authenticated path.
pub(crate) async fn subscribe(
    State(state): State<Arc<ServeState>>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> Response {
    let caller = match identified(&state, &headers) {
        Ok(caller) => caller,
        Err(response) => return response,
    };
    if body["v"].as_str() != Some(V) {
        return refused(
            StatusCode::UNPROCESSABLE_ENTITY,
            "invalid_request",
            format!("v must be {V}"),
        );
    }
    let topics = match body["topics"].as_object() {
        Some(topics)
            if topics.get("product").is_some_and(Value::is_boolean)
                && topics.get("support").is_some_and(Value::is_boolean) =>
        {
            json!({
                "product": topics["product"],
                "support": topics["support"],
            })
        }
        _ => {
            return refused(
                StatusCode::UNPROCESSABLE_ENTITY,
                "invalid_request",
                "topics.product and topics.support booleans are required".into(),
            );
        }
    };
    let contact = match body.get("contact") {
        None | Some(Value::Null) => None,
        Some(contact) => {
            let Some(email) = contact["email"].as_str() else {
                return refused(
                    StatusCode::UNPROCESSABLE_ENTITY,
                    "invalid_request",
                    "contact.email is the only supported contact".into(),
                );
            };
            if email.len() > MAX_CONTACT || !email.contains('@') {
                return refused(
                    StatusCode::UNPROCESSABLE_ENTITY,
                    "invalid_request",
                    "contact.email must be an email-shaped string of at most 256 bytes".into(),
                );
            }
            Some(email.to_string())
        }
    };
    let mut record = read(&state.dir, &caller).unwrap_or_else(|| {
        json!({
            "v": V,
            "subscription": subscription_id(&caller),
            "credential": caller,
            "subscribed": now_utc(),
        })
    });
    record["status"] = json!("subscribed");
    record["topics"] = topics;
    record["verified"] = json!("credential");
    record["updated"] = json!(now_utc());
    if let Some(email) = contact {
        record["contact"] = json!({"email": email});
    } else {
        record.as_object_mut().unwrap().remove("contact");
    }
    match persist(&state.dir, &caller, &record) {
        Ok(()) => (StatusCode::OK, Json(record)).into_response(),
        Err(trouble) => refused(
            StatusCode::INTERNAL_SERVER_ERROR,
            "subscription_unavailable",
            trouble,
        ),
    }
}

/// `GET /v1/updates`: the caller's own subscription, or `404
/// subscription_not_found` — the same answer an unsubscribed caller gets, so
/// existence is not disclosed to anyone but the credential holder.
pub(crate) async fn view(State(state): State<Arc<ServeState>>, headers: HeaderMap) -> Response {
    let caller = match identified(&state, &headers) {
        Ok(caller) => caller,
        Err(response) => return response,
    };
    match read(&state.dir, &caller) {
        Some(record) => Json(record).into_response(),
        None => refused(
            StatusCode::NOT_FOUND,
            "subscription_not_found",
            "this credential holds no updates subscription".into(),
        ),
    }
}

/// `DELETE /v1/updates`: unsubscribe. The record survives with
/// `status: unsubscribed` — the durable proof the opt-out was honored.
pub(crate) async fn unsubscribe(
    State(state): State<Arc<ServeState>>,
    headers: HeaderMap,
) -> Response {
    let caller = match identified(&state, &headers) {
        Ok(caller) => caller,
        Err(response) => return response,
    };
    let Some(mut record) = read(&state.dir, &caller) else {
        return refused(
            StatusCode::NOT_FOUND,
            "subscription_not_found",
            "this credential holds no updates subscription".into(),
        );
    };
    record["status"] = json!("unsubscribed");
    record["updated"] = json!(now_utc());
    match persist(&state.dir, &caller, &record) {
        Ok(()) => Json(record).into_response(),
        Err(trouble) => refused(
            StatusCode::INTERNAL_SERVER_ERROR,
            "subscription_unavailable",
            trouble,
        ),
    }
}

/// The credential the subscription binds to — an identified tenant key
/// or session. An anonymous caller cannot subscribe: an unverified
/// opt-in is no opt-in.
fn identified(state: &ServeState, headers: &HeaderMap) -> Result<String, Response> {
    match authenticate(state, headers) {
        Ok((_registry, caller)) if caller.tenant.is_some() || caller.key.starts_with("sess_") => {
            Ok(caller.key)
        }
        Ok(_) => Err(refused(
            StatusCode::UNAUTHORIZED,
            "unauthenticated",
            "an identified credential is required to subscribe".into(),
        )),
        Err((status, code, message)) => Err(refused(status, code, message)),
    }
}

/// One record per credential: `updates/<digest>.json`, named by digest so
/// no credential id ever becomes a path component.
fn record_path(root: &Path, credential: &str) -> PathBuf {
    root.join("updates")
        .join(format!("{}.json", digest(credential)))
}

fn subscription_id(credential: &str) -> String {
    format!("sub_{}", &digest(credential)[..16])
}

fn digest(credential: &str) -> String {
    use sha2::Digest;
    let digest = sha2::Sha256::digest(credential.as_bytes());
    format!("{digest:x}")
}

fn read(root: &Path, credential: &str) -> Option<Value> {
    let text = fs::read_to_string(record_path(root, credential)).ok()?;
    serde_json::from_str(&text).ok()
}

fn persist(root: &Path, credential: &str, record: &Value) -> Result<(), String> {
    let path = record_path(root, credential);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|trouble| trouble.to_string())?;
    }
    let scratch = path.with_extension("tmp");
    fs::write(&scratch, record.to_string()).map_err(|trouble| trouble.to_string())?;
    fs::rename(&scratch, &path).map_err(|trouble| trouble.to_string())
}

fn refused(status: StatusCode, code: &str, message: String) -> Response {
    (
        status,
        Json(json!({
            "error": {"code": code, "message": message}
        })),
    )
        .into_response()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_subscription_round_trips_and_unsubscribes() {
        let dir = tempfile::tempdir().unwrap();
        let record = json!({
            "v": V,
            "subscription": subscription_id("oak_abc"),
            "credential": "oak_abc",
            "status": "subscribed",
            "topics": {"product": true, "support": false},
            "verified": "credential",
        });
        persist(dir.path(), "oak_abc", &record).unwrap();
        let loaded = read(dir.path(), "oak_abc").unwrap();
        assert_eq!(loaded["status"], "subscribed");
        assert_eq!(loaded["topics"]["support"], false);
        assert!(read(dir.path(), "oak_other").is_none());

        let mut updated = loaded;
        updated["status"] = json!("unsubscribed");
        persist(dir.path(), "oak_abc", &updated).unwrap();
        assert_eq!(
            read(dir.path(), "oak_abc").unwrap()["status"],
            "unsubscribed"
        );
    }
}
