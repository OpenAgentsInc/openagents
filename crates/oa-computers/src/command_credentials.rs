//! Short-lived, account-bound credentials for one command. Only the trusted
//! API service can prepare them; public requests carry the reference alone.

use crate::api::{ACCOUNT_HEADER, Api};
use axum::{
    Json,
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
};
use computer_driver::Secret;
use serde::Deserialize;
use serde_json::json;
use std::{
    collections::{BTreeMap, HashMap},
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

pub fn valid_reference(reference: &str) -> bool {
    reference.len() <= 128
        && reference.strip_prefix("cred_").is_some_and(|tail| {
            !tail.is_empty()
                && tail
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b"_-".contains(&b))
        })
}

#[derive(Clone, Default)]
pub struct Pending(Arc<Mutex<HashMap<String, Entry>>>);
struct Entry {
    owner: String,
    computer: String,
    request: String,
    command_sha256: String,
    expires: Instant,
    values: BTreeMap<String, Secret>,
}
impl Pending {
    pub fn prepare(
        &self,
        reference: &str,
        owner: &str,
        computer: &str,
        request: &str,
        command_sha256: &str,
        values: BTreeMap<String, Secret>,
    ) -> Result<(), ()> {
        if command_sha256.len() != 64
            || !command_sha256.bytes().all(|b| b.is_ascii_hexdigit())
            || !valid_reference(reference)
            || !coder_environment::valid_id(request)
            || values.is_empty()
            || values.len() > 16
            || values.iter().any(|(name, value)| {
                !computer_driver::env_name_ok(name)
                    || value.expose().len() > 65536
                    || value.expose().contains('\0')
            })
        {
            return Err(());
        }
        let mut entries = self.0.lock().unwrap_or_else(|e| e.into_inner());
        entries.retain(|_, e| e.expires > Instant::now());
        if let Some(old) = entries.get(reference) {
            return if old.owner == owner
                && old.computer == computer
                && old.request == request
                && old.command_sha256 == command_sha256
                && old.values.len() == values.len()
                && old
                    .values
                    .iter()
                    .all(|(k, v)| values.get(k).is_some_and(|n| n.expose() == v.expose()))
            {
                Ok(())
            } else {
                Err(())
            };
        }
        let held_bytes: usize = entries
            .values()
            .flat_map(|entry| entry.values.values())
            .map(|value| value.expose().len())
            .sum();
        let new_bytes: usize = values.values().map(|value| value.expose().len()).sum();
        if entries.len() >= 256 || held_bytes.saturating_add(new_bytes) > 8 * 1024 * 1024 {
            return Err(());
        }
        entries.insert(
            reference.into(),
            Entry {
                owner: owner.into(),
                computer: computer.into(),
                request: request.into(),
                command_sha256: command_sha256.into(),
                expires: Instant::now() + Duration::from_secs(600),
                values,
            },
        );
        Ok(())
    }
    pub fn names(
        &self,
        reference: &str,
        owner: &str,
        computer: &str,
        request: &str,
    ) -> Option<Vec<String>> {
        let entries = self.0.lock().unwrap_or_else(|e| e.into_inner());
        let e = entries.get(reference)?;
        (e.expires > Instant::now()
            && e.owner == owner
            && e.computer == computer
            && e.request == request)
            .then(|| e.values.keys().cloned().collect())
    }
    pub fn take(
        &self,
        reference: &str,
        owner: &str,
        computer: &str,
        request: &str,
        command_sha256: &str,
    ) -> Option<BTreeMap<String, Secret>> {
        let mut entries = self.0.lock().unwrap_or_else(|e| e.into_inner());
        let e = entries.get(reference)?;
        if e.expires <= Instant::now()
            || e.owner != owner
            || e.computer != computer
            || e.request != request
            || e.command_sha256 != command_sha256
        {
            return None;
        }
        entries.remove(reference).map(|e| e.values)
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Prepare {
    reference: String,
    request: String,
    command_sha256: String,
    values: BTreeMap<String, String>,
}
pub async fn prepare(
    State(api): State<Arc<Api>>,
    headers: HeaderMap,
    Path(computer): Path<String>,
    body: axum::body::Bytes,
) -> Response {
    let Some(owner) = headers.get(ACCOUNT_HEADER).and_then(|v| v.to_str().ok()) else {
        return StatusCode::BAD_REQUEST.into_response();
    };
    if body.len() > 256 * 1024 {
        return StatusCode::PAYLOAD_TOO_LARGE.into_response();
    }
    let Ok(sent) = serde_json::from_slice::<Prepare>(&body) else {
        return StatusCode::BAD_REQUEST.into_response();
    };
    drop(body);
    match api.store.get(owner, &computer).await {
        Ok(Some(e)) if e.desired != crate::store::Desired::Deleted => {}
        Ok(_) => return StatusCode::NOT_FOUND.into_response(),
        Err(_) => return StatusCode::SERVICE_UNAVAILABLE.into_response(),
    }
    let values = sent
        .values
        .into_iter()
        .map(|(k, v)| (k, Secret::new(v)))
        .collect();
    match api.turns.2.prepare(
        &sent.reference,
        owner,
        &computer,
        &sent.request,
        &sent.command_sha256,
        values,
    ) {
        Ok(()) => (
            StatusCode::CREATED,
            Json(json!({"reference":sent.reference})),
        )
            .into_response(),
        Err(()) => StatusCode::CONFLICT.into_response(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn credentials_are_bound_to_account_computer_request_and_consumed_once() {
        let p = Pending::default();
        let values = || BTreeMap::from([("ANTHROPIC_API_KEY".into(), Secret::new("fixture"))]);
        p.prepare(
            "cred_test",
            "one",
            "cmp_one",
            "request_one",
            &"a".repeat(64),
            values(),
        )
        .unwrap();
        assert!(
            p.names("cred_test", "two", "cmp_one", "request_one")
                .is_none()
        );
        assert!(
            p.names("cred_test", "one", "cmp_two", "request_one")
                .is_none()
        );
        assert!(
            p.take(
                "cred_test",
                "one",
                "cmp_one",
                "request_two",
                &"a".repeat(64)
            )
            .is_none()
        );
        assert!(
            p.prepare(
                "cred_test",
                "two",
                "cmp_one",
                "request_one",
                &"a".repeat(64),
                values()
            )
            .is_err()
        );
        assert!(
            p.take(
                "cred_test",
                "one",
                "cmp_one",
                "request_one",
                &"a".repeat(64)
            )
            .is_some()
        );
        assert!(
            p.take(
                "cred_test",
                "one",
                "cmp_one",
                "request_one",
                &"a".repeat(64)
            )
            .is_none()
        );
    }
}
