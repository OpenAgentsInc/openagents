//! Sign in with Apple (#11980): the identity token from the phone's
//! native Apple sheet, exchanged at `POST /v1/device/apple` for the same
//! app session a device sign-in issues.

use std::fmt;

use reqwest::Method;
use serde_json::{Value, json};

use super::{CredentialRecord, Session, TokenAnswer};
use crate::{Client, Error};

/// What the Sign in with Apple sheet returned, plus the app's own label.
#[derive(Clone)]
pub struct AppleSignIn {
    /// Apple's identity token (a JWT), as the sheet returned it.
    pub identity_token: String,
    /// The random value whose SHA-256 (hex) the app gave Apple's request.
    pub nonce: String,
    /// The name the sheet returned; Apple sends it only the first time.
    pub name: Option<String>,
    /// The app's name, such as `OpenAgents`.
    pub app: String,
    /// This device's name.
    pub computer: String,
}

impl fmt::Debug for AppleSignIn {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AppleSignIn")
            .field("app", &self.app)
            .field("computer", &self.computer)
            .finish_non_exhaustive()
    }
}

impl Client {
    /// Exchange an Apple identity token for an app session. Sends once;
    /// the caller stores the session as it stores a device sign-in's.
    pub async fn sign_in_with_apple(&self, request: &AppleSignIn) -> Result<Session, Error> {
        if request.identity_token.is_empty()
            || request.identity_token.len() > 8192
            || request.nonce.is_empty()
            || request.nonce.len() > 256
        {
            return Err(Error::InvalidCredential);
        }
        let mut body = json!({
            "identity_token": request.identity_token,
            "nonce": request.nonce,
            "app": request.app,
            "computer": request.computer,
        });
        if let Some(name) = request.name.as_deref().filter(|n| !n.trim().is_empty()) {
            body["name"] = json!(name);
        }
        let answer = match self
            .request::<Value>(Method::POST, &["device", "apple"], &[], Some(body), None)
            .await
        {
            Ok(answer) => answer,
            Err(Error::Unauthorized | Error::Refused { status: 400, .. }) => {
                return Err(Error::AppleDenied);
            }
            Err(error) => return Err(error),
        };
        let answer: TokenAnswer =
            serde_json::from_value(answer).map_err(|_| Error::InvalidResponse)?;
        answer.checked()?;
        Ok(Session {
            record: CredentialRecord {
                deployment: self.deployment.clone(),
                account: answer.account,
                expires_at: Some(crate::now().saturating_add(answer.expires_in)),
                credential: answer.access_token,
            },
        })
    }
}
