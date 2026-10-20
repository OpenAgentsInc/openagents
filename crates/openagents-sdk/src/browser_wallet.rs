//! The browser's phone-approved wallet requests, as the phone reads and
//! answers them (#12258, #12306): `GET /v1/wallet/browser`, `PUT
//! /v1/wallet/browser/grant`, and `POST
//! /v1/wallet/browser/requests/{id}/receipt`, under the app's session.
//!
//! The grant, request, and receipt are the agent spend protocol's
//! documents (`openagents.spend-grant.v1`, `openagents.spend-request.v1`,
//! `openagents.spend-receipt.v1`, `coder_access::spend`), carried here as
//! JSON so this crate stays free of that one.
use reqwest::Method;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::{Client, Error, Session};

/// One request the browser made and the phone's receipt, if any.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
pub struct BrowserRequest {
    pub request: Value,
    #[serde(default)]
    pub receipt: Option<Value>,
}

/// What the site holds for this account's browser.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
pub struct BrowserWallet {
    /// The browser's key, which a grant names as its grantee.
    pub grantee: String,
    /// The phone's current grant, if it gave one.
    #[serde(default)]
    pub grant: Option<Value>,
    #[serde(default)]
    pub requests: Vec<BrowserRequest>,
}

fn valid_id(id: &str) -> bool {
    id.len() == 64
        && id
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

impl Client {
    /// The browser's grantee key, the grant, and the requests.
    pub async fn browser_wallet(&self, session: &Session) -> Result<BrowserWallet, Error> {
        self.check_session(session)?;
        self.request(
            Method::GET,
            &["wallet", "browser"],
            &[],
            None,
            Some(session.token()),
        )
        .await
    }

    /// Give the browser the bounds it may ask within.
    pub async fn grant_browser_wallet(
        &self,
        session: &Session,
        grant: Value,
    ) -> Result<Value, Error> {
        self.check_session(session)?;
        self.request(
            Method::PUT,
            &["wallet", "browser", "grant"],
            &[],
            Some(json!({ "grant": grant })),
            Some(session.token()),
        )
        .await
    }

    /// Answer one of the browser's requests.
    pub async fn settle_browser_request(
        &self,
        session: &Session,
        request: &str,
        receipt: Value,
    ) -> Result<Value, Error> {
        self.check_session(session)?;
        if !valid_id(request) {
            return Err(Error::NotFound);
        }
        self.request(
            Method::POST,
            &["wallet", "browser", "requests", request, "receipt"],
            &[],
            Some(json!({ "receipt": receipt })),
            Some(session.token()),
        )
        .await
    }
}
