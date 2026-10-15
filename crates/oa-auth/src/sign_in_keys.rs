//! The account service's sign-in key routes (#11305), as plain functions
//! over the session store, so the gateway and the local fixture's account
//! service answer the same way.
//!
//! | Route | Function |
//! | --- | --- |
//! | `GET /v1/account/sign-in-keys` | [`list`] |
//! | `POST /v1/account/sign-in-keys` `{name, expires_in_days?}` | [`create`] |
//! | `DELETE /v1/account/sign-in-keys/{id}` | [`revoke`] |
//!
//! Each runs as an account a session or sign-in key resolved to. A key is
//! a user session (`tenancy::sessions::keys`); its token appears once, in
//! [`create`]'s answer, and no answer here carries another key's secret,
//! because none is kept.

use std::path::Path;

use serde_json::{Value, json};
use tenancy::sessions::{self, KeyLabel, Refusal, SessionId, Sessions};
use tenancy::workspaces::UserId;

use crate::device::Answer;

/// The longest expiry a key may be made with, in days.
pub const MAX_DAYS: u64 = 365;

fn ok(mut body: Value) -> Answer {
    body["v"] = json!("openagents.accounts.v1");
    Answer { status: 200, body }
}

fn refused(status: u16, code: &str, message: impl Into<String>) -> Answer {
    Answer {
        status,
        body: json!({"error": {"code": code, "message": message.into()}}),
    }
}

fn unavailable() -> Answer {
    refused(
        503,
        "sessions_unavailable",
        "Login keys aren't available right now. Try again in a minute.",
    )
}

fn unknown() -> Answer {
    refused(404, "unknown_key", "There's no working key with that id.")
}

/// One key as the routes show it: never its secret.
#[must_use]
pub fn key_json(session: &sessions::Session, current: Option<&str>) -> Value {
    let label = session.key.as_ref();
    json!({
        "id": session.id.as_str(),
        "name": label.map(|l| l.name.as_str()).unwrap_or_default(),
        "created_at": session.created_at,
        "last_used_at": label.and_then(|l| l.last_used),
        "expires_at": (session.expires_at < sessions::KEY_NEVER).then_some(session.expires_at),
        "current": current == Some(session.id.as_str()),
    })
}

/// `GET /v1/account/sign-in-keys` — `{keys: [...]}`, newest first.
pub fn list(dir: &Path, account: &str, session: Option<&str>) -> Answer {
    let Ok(store) = Sessions::open(dir).and_then(|s| s.store()) else {
        return unavailable();
    };
    let keys: Vec<Value> = store
        .book
        .sign_in_keys(&UserId::from(account), now())
        .into_iter()
        .map(|key| key_json(key, session))
        .collect();
    ok(json!({ "keys": keys }))
}

/// `POST /v1/account/sign-in-keys` — `{name, expires_in_days?}`; answers
/// `{key, token}`. `expires_in_days` absent or null makes a key that
/// works until revoked.
pub fn create(dir: &Path, account: &str, session: Option<&str>, body: &Value) -> Answer {
    let name = body.get("name").and_then(Value::as_str).unwrap_or_default();
    let label = match KeyLabel::new(name) {
        Ok(label) => label,
        Err(refusal) => return refused(400, "invalid_name", capitalized(&refusal)),
    };
    let ttl = match body.get("expires_in_days") {
        None | Some(Value::Null) => None,
        Some(days) => match days.as_u64().filter(|d| (1..=MAX_DAYS).contains(d)) {
            Some(days) => Some(days * 86_400),
            None => {
                return refused(
                    400,
                    "invalid_expiry",
                    format!("Pick an expiry of 1 to {MAX_DAYS} days, or none."),
                );
            }
        },
    };
    let Ok(store) = Sessions::open(dir) else {
        return unavailable();
    };
    let user = UserId::from(account);
    match store.mutate(|book, log, now| {
        let issued = book.issue_key(user.clone(), label, ttl, now)?;
        sessions::push_access(
            log,
            sessions::Access {
                at: now,
                actor: account.to_string(),
                action: "sign-in-key-create".into(),
                workspace: None,
                session: session.map(str::to_string),
                detail: Some(issued.session.id.as_str().to_string()),
            },
        );
        Ok(issued)
    }) {
        Ok(issued) => ok(json!({
            "key": key_json(&issued.session, session),
            "token": issued.once,
        })),
        Err(refusal @ Refusal::KeyLimit) => refused(409, "too_many_keys", capitalized(&refusal)),
        Err(_) => unavailable(),
    }
}

/// `DELETE /v1/account/sign-in-keys/{id}` — revoke one of the account's
/// own keys. Answers `{key: {id, state: "revoked"}}`.
pub fn revoke(dir: &Path, account: &str, session: Option<&str>, id: &str) -> Answer {
    if id.len() != 64 || !id.bytes().all(|b| b.is_ascii_hexdigit()) {
        return unknown();
    }
    let Ok(store) = Sessions::open(dir) else {
        return unavailable();
    };
    let user = UserId::from(account);
    match store.mutate(|book, log, now| {
        let ended = book.revoke_key(&user, &SessionId::from(id), now)?;
        sessions::push_access(
            log,
            sessions::Access {
                at: now,
                actor: account.to_string(),
                action: "sign-in-key-revoke".into(),
                workspace: None,
                session: session.map(str::to_string),
                detail: Some(ended.id.as_str().to_string()),
            },
        );
        Ok(ended)
    }) {
        Ok(ended) => ok(json!({"key": {"id": ended.id.as_str(), "state": "revoked"}})),
        Err(Refusal::UnknownSession(_) | Refusal::SessionClosed { .. }) => unknown(),
        Err(_) => unavailable(),
    }
}

/// Note a use of the key whose session id is `session`: written at most
/// hourly ([`sessions::KEY_LAST_USED_EVERY`]), so the read that resolved
/// it pays one store write an hour at most. Best effort: a store that
/// can't be written leaves the last use as it was.
pub fn note_use(dir: &Path, store: &sessions::Store, session: &str) {
    let id = SessionId::from(session);
    if !store.book.key_use_stale(&id, now()) {
        return;
    }
    if let Ok(sessions) = Sessions::open(dir) {
        let _ = sessions.mutate(|book, _, now| Ok(book.note_key_use(&id, now)));
    }
}

fn capitalized(refusal: &Refusal) -> String {
    let text = refusal.to_string();
    let mut chars = text.chars();
    chars.next().map_or_else(String::new, |first| {
        first.to_uppercase().collect::<String>() + chars.as_str() + "."
    })
}

fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tenancy::sessions::SessionBook;

    fn store() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        Sessions::install(dir.path(), SessionBook::new(3_600, 600)).unwrap();
        dir
    }

    #[test]
    fn create_list_revoke_and_no_secret_in_listing() {
        let dir = store();
        let made = create(
            dir.path(),
            "acct_me",
            None,
            &json!({"name": "ci", "expires_in_days": 30}),
        );
        assert_eq!(made.status, 200, "{}", made.body);
        let token = made.body["token"].as_str().unwrap().to_string();
        assert!(token.starts_with("oa_sk_"));
        let id = made.body["key"]["id"].as_str().unwrap().to_string();
        assert!(made.body["key"]["expires_at"].as_u64().is_some());

        let listed = list(dir.path(), "acct_me", Some(&id));
        assert_eq!(listed.body["keys"].as_array().unwrap().len(), 1);
        assert_eq!(listed.body["keys"][0]["current"], json!(true));
        assert!(!listed.body.to_string().contains(&token));
        assert!(
            list(dir.path(), "acct_other", None).body["keys"]
                .as_array()
                .unwrap()
                .is_empty()
        );

        assert_eq!(revoke(dir.path(), "acct_other", None, &id).status, 404);
        assert_eq!(revoke(dir.path(), "acct_me", None, &id).status, 200);
        assert_eq!(revoke(dir.path(), "acct_me", None, &id).status, 404);
        assert!(
            list(dir.path(), "acct_me", None).body["keys"]
                .as_array()
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn never_expiring_and_bad_requests() {
        let dir = store();
        let made = create(dir.path(), "acct_me", None, &json!({"name": "agent"}));
        assert_eq!(made.body["key"]["expires_at"], Value::Null);
        assert_eq!(
            create(dir.path(), "acct_me", None, &json!({"name": ""})).status,
            400
        );
        assert_eq!(
            create(
                dir.path(),
                "acct_me",
                None,
                &json!({"name": "a", "expires_in_days": 0})
            )
            .status,
            400
        );
    }

    #[test]
    fn use_is_noted() {
        let dir = store();
        let made = create(dir.path(), "acct_me", None, &json!({"name": "agent"}));
        let id = made.body["key"]["id"].as_str().unwrap().to_string();
        let store = Sessions::open(dir.path()).unwrap().store().unwrap();
        note_use(dir.path(), &store, &id);
        let listed = list(dir.path(), "acct_me", None);
        assert!(listed.body["keys"][0]["last_used_at"].as_u64().is_some());
    }
}
