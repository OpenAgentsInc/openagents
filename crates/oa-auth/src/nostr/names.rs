//! `name@openagents.com` names over the account book (#12254,
//! `tenancy::accounts::names`).
//!
//! | Route | Caller | Function |
//! | --- | --- | --- |
//! | `GET /v1/account/name` | a session | [`NostrAuth::name`] |
//! | `PUT /v1/account/name` `{name, pubkey?}` | a session | [`NostrAuth::claim_name`] |
//! | `DELETE /v1/account/name` | a session | [`NostrAuth::release_name`] |
//! | `GET /v1/names/{name}` | anyone | [`NostrAuth::lookup_name`] |
//! | `GET /v1/names?pubkey=HEX` or `?github=LOGIN` | anyone | [`NostrAuth::lookup_key`], [`NostrAuth::lookup_github`] |
//!
//! A public answer names only the name and the key it points at: both are
//! public by design, since `/.well-known/nostr.json` serves them.

use serde_json::{Value, json};
use tenancy::Accounts;
use tenancy::accounts::names::{self, Claimed};

use super::{NostrAuth, ok, parse_pubkey, refused, unavailable};
use crate::device::Answer;

fn view(name: &str, pubkey: &str) -> Value {
    json!({"name": name, "pubkey": pubkey})
}

fn not_found() -> Answer {
    refused(404, "not_found", "Nobody has that name.")
}

impl NostrAuth {
    fn book(&self) -> Option<tenancy::accounts::Store> {
        Accounts::open(&self.dir).and_then(|a| a.store()).ok()
    }

    /// `GET /v1/account/name` as `account`: `{"name": null}` or its name
    /// and key.
    #[must_use]
    pub fn name(&self, account: &str) -> Answer {
        let Some(store) = self.book() else {
            return unavailable();
        };
        match names::name_of(&store, account) {
            Some((name, claim)) => ok(view(&name, &claim.pubkey)),
            None => ok(json!({"name": null})),
        }
    }

    /// `PUT /v1/account/name` `{"name", "pubkey"?}` as `account`.
    #[must_use]
    pub fn claim_name(&self, account: &str, body: &[u8]) -> Answer {
        let Ok(body) = serde_json::from_slice::<Value>(body) else {
            return refused(400, "invalid_request", "Send the name you want.");
        };
        let Some(name) = body.get("name").and_then(Value::as_str) else {
            return refused(400, "invalid_request", "Send the name you want.");
        };
        let pubkey = match body.get("pubkey").and_then(Value::as_str) {
            Some(text) => match parse_pubkey(text) {
                Some(key) => Some(key),
                None => {
                    let problem = names::NameProblem::NotYourKey;
                    return refused(400, problem.code(), problem.message());
                }
            },
            None => None,
        };
        let Ok(accounts) = Accounts::open(&self.dir) else {
            return unavailable();
        };
        match accounts.claim_name(account, name, pubkey.as_deref()) {
            Ok(Claimed::Done { name, claim }) => ok(view(&name, &claim.pubkey)),
            Ok(Claimed::Refused(problem)) => {
                let status = match problem {
                    names::NameProblem::Taken => 409,
                    names::NameProblem::TooOften => 429,
                    _ => 400,
                };
                refused(status, problem.code(), problem.message())
            }
            Err(tenancy::accounts::Refusal::UnknownAccount(_)) => {
                refused(401, "unauthenticated", "Log in again.")
            }
            Err(_) => unavailable(),
        }
    }

    /// `DELETE /v1/account/name` as `account`.
    #[must_use]
    pub fn release_name(&self, account: &str) -> Answer {
        let Ok(accounts) = Accounts::open(&self.dir) else {
            return unavailable();
        };
        match accounts.release_name(account) {
            Ok(released) => ok(json!({"released": released})),
            Err(_) => unavailable(),
        }
    }

    /// `GET /v1/names/{name}`: the key a name points at.
    #[must_use]
    pub fn lookup_name(&self, name: &str) -> Answer {
        let Some(store) = self.book() else {
            return unavailable();
        };
        match names::resolve(&store, name) {
            Some(claim) => ok(view(&names::normalize(name), &claim.pubkey)),
            None => not_found(),
        }
    }

    /// `GET /v1/names?pubkey=HEX`: the name that points at a key.
    #[must_use]
    pub fn lookup_key(&self, pubkey: &str) -> Answer {
        let Some(pubkey) = parse_pubkey(pubkey) else {
            return not_found();
        };
        let Some(store) = self.book() else {
            return unavailable();
        };
        match names::name_for_key(&store, &pubkey) {
            Some(name) => ok(view(&name, &pubkey)),
            None => not_found(),
        }
    }

    /// `GET /v1/names?github=LOGIN`: the public half of the account a
    /// GitHub login signs in (the login, its display name, and when it
    /// joined, as the old `/u/{login}` page showed), with its name and key
    /// when it has one, so `/u/{login}` can move to `/@{name}`.
    #[must_use]
    pub fn lookup_github(&self, login: &str) -> Answer {
        let Some(store) = self.book() else {
            return unavailable();
        };
        let Some(identity) = store
            .identities
            .github
            .values()
            .find(|identity| identity.profile.login.eq_ignore_ascii_case(login))
        else {
            return not_found();
        };
        let held = names::name_of(&store, &identity.account);
        ok(json!({
            "github": {
                "login": identity.profile.login,
                "name": identity.profile.name,
                "joined": identity.linked,
            },
            "name": held.as_ref().map(|(name, _)| name.clone()),
            "pubkey": held.as_ref().map(|(_, claim)| claim.pubkey.clone()),
        }))
    }

    /// `GET /v1/names` with `pubkey` or `github` in the query.
    #[must_use]
    pub fn lookup_query(&self, query: Option<&str>) -> Answer {
        let pairs = url_pairs(query.unwrap_or_default());
        if let Some(pubkey) = pairs.iter().find(|(k, _)| k == "pubkey") {
            return self.lookup_key(&pubkey.1);
        }
        if let Some(login) = pairs.iter().find(|(k, _)| k == "github") {
            return self.lookup_github(&login.1);
        }
        refused(400, "invalid_request", "Ask with `pubkey` or `github`.")
    }
}

/// `a=b&c=d` pairs; values here are hex keys and GitHub logins, which
/// need no percent-decoding beyond what their shape allows.
fn url_pairs(query: &str) -> Vec<(String, String)> {
    query
        .split('&')
        .filter_map(|pair| pair.split_once('='))
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect()
}
