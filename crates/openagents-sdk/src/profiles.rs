//! Public profiles and published agents' owners (#12254, #12255).
//!
//! A person's profile is a kind 0 event their own key signs; the apps
//! publish it to a relay themselves. What this module calls is the rest:
//!
//! - The account's `name@openagents.com` name, as the app's session
//!   (`GET`, `PUT`, and `DELETE /v1/device/name`).
//! - A published agent's own key and the NIP-OA owner tag the owner's
//!   client signs for it (`GET`, `PUT /v1/published-agents/{id}/owner`).
use reqwest::Method;
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::{Client, Error, Session};

/// The account's name and the key it points at, or no name.
#[derive(Clone, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
pub struct AccountName {
    /// The name before `@openagents.com`.
    #[serde(default)]
    pub name: Option<String>,
    /// The key it points at, 64 lowercase hex characters.
    #[serde(default)]
    pub pubkey: Option<String>,
}

/// What the owner of a published agent signs, and who owns it now.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub struct AgentOwner {
    /// The agent's own key, 64 lowercase hex characters.
    pub agent: String,
    /// The agent's key as `npub1…`.
    pub npub: String,
    /// The NIP-OA conditions to sign, such as `created_at<1800000000`.
    pub conditions: String,
    /// The exact text the owner's signature covers.
    pub preimage: String,
    /// The owner's key, once the agent has one.
    #[serde(default)]
    pub owner: Option<String>,
    /// Whether the owner retired the agent.
    #[serde(default)]
    pub revoked: bool,
}

/// The answer after the owner's tag was accepted.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub struct OwnerSet {
    /// The agent's own key.
    pub agent: String,
    /// The agent's key as `npub1…`.
    pub npub: String,
    /// The owner's key.
    pub owner: String,
    /// How many relays took the agent's profile and definition.
    #[serde(default)]
    pub published: u64,
}

fn valid_published(id: &str) -> bool {
    id.len() == 64
        && id
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

impl Client {
    /// The signed-in account's `name@openagents.com` name.
    pub async fn account_name(&self, session: &Session) -> Result<AccountName, Error> {
        self.check_session(session)?;
        self.request(
            Method::GET,
            &["device", "name"],
            &[],
            None,
            Some(session.token()),
        )
        .await
    }

    /// Claim or change the account's name, pointing it at `pubkey` (one of
    /// the account's keys) when given. A refusal carries the name's
    /// problem as its code: `name_invalid`, `name_reserved`, `name_taken`,
    /// `name_too_often`, or `key_not_yours`.
    pub async fn claim_account_name(
        &self,
        session: &Session,
        name: &str,
        pubkey: Option<&str>,
    ) -> Result<AccountName, Error> {
        self.check_session(session)?;
        let mut body = json!({ "name": name });
        if let Some(pubkey) = pubkey {
            body["pubkey"] = json!(pubkey);
        }
        self.request(
            Method::PUT,
            &["device", "name"],
            &[],
            Some(body),
            Some(session.token()),
        )
        .await
    }

    /// The key of the published agent `id` and what its owner signs.
    /// Anyone can read it.
    pub async fn published_agent_owner(&self, id: &str) -> Result<AgentOwner, Error> {
        if !valid_published(id) {
            return Err(Error::InvalidReference);
        }
        self.request(
            Method::GET,
            &["published-agents", id, "owner"],
            &[],
            None,
            None,
        )
        .await
    }

    /// Set the owner of the agent you published as `id` with the
    /// `["auth", owner, conditions, signature]` tag the owner's key signed.
    /// The site then publishes the agent's profile and definition.
    pub async fn set_published_agent_owner(
        &self,
        session: &Session,
        id: &str,
        auth: &[String],
    ) -> Result<OwnerSet, Error> {
        self.check_session(session)?;
        if !valid_published(id) || auth.len() != 4 || auth[0] != "auth" {
            return Err(Error::InvalidReference);
        }
        self.request(
            Method::PUT,
            &["published-agents", id, "owner"],
            &[],
            Some(json!({ "auth": auth })),
            Some(session.token()),
        )
        .await
    }
}

/// A browser's Verse key waiting for, or carrying, the owner tag the
/// person's own key signs (#12313).
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub struct VerseKey {
    /// The browser's Verse key, 64 lowercase hex characters.
    pub pubkey: String,
    /// The NIP-OA conditions to sign, such as `created_at<1800000000`.
    pub conditions: String,
    /// The exact text the owner's signature covers.
    pub preimage: String,
    /// The signed tag, once there is one.
    #[serde(default)]
    pub auth: Option<Vec<String>>,
}

impl Client {
    /// The account's browser Verse key, if a browser offered one.
    pub async fn verse_key(&self, session: &Session) -> Result<Option<VerseKey>, Error> {
        self.check_session(session)?;
        match self
            .request(
                Method::GET,
                &["verse", "key"],
                &[],
                None,
                Some(session.token()),
            )
            .await
        {
            Ok(key) => Ok(Some(key)),
            Err(Error::NotFound | Error::Refused { status: 404, .. }) => Ok(None),
            Err(error) => Err(error),
        }
    }

    /// Own the browser's Verse key with `auth`, the NIP-OA tag this
    /// account's key signed over it.
    pub async fn own_verse_key(
        &self,
        session: &Session,
        auth: &[String],
    ) -> Result<VerseKey, Error> {
        self.check_session(session)?;
        self.request(
            Method::PUT,
            &["verse", "key"],
            &[],
            Some(json!({ "auth": auth })),
            Some(session.token()),
        )
        .await
    }
}
