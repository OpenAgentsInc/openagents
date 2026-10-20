//! Account-owned Lightning addresses. Wallet proofs are signed on the device.
use crate::{Client, Error, Session};
use reqwest::Method;
use serde::{Deserialize, Serialize};
use serde_json::json;

#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AddressState {
    Pending,
    Active,
    Disabled,
    Retired,
}
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub struct LightningAddress {
    pub name: String,
    pub account: String,
    pub nostr_pubkey: String,
    pub spark_pubkey: String,
    pub domain: String,
    pub revision: u64,
    pub state: AddressState,
    pub created: u64,
    pub updated: u64,
    pub last_challenge: String,
}
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub struct LightningAddresses {
    pub configured: bool,
    pub addresses: Vec<LightningAddress>,
}
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
pub struct LightningChallenge {
    pub id: String,
    pub account: String,
    pub name: String,
    pub domain: String,
    pub operation: String,
    pub expected_revision: u64,
    pub expires_at: u64,
    pub message: String,
}
impl Client {
    pub async fn lightning_addresses(
        &self,
        session: &Session,
    ) -> Result<LightningAddresses, Error> {
        self.check_session(session)?;
        self.request(
            Method::GET,
            &["device", "lightning-address"],
            &[],
            None,
            Some(session.token()),
        )
        .await
    }
    pub async fn lightning_challenge(
        &self,
        session: &Session,
        expected_revision: u64,
    ) -> Result<LightningChallenge, Error> {
        self.check_session(session)?;
        self.request(
            Method::POST,
            &["device", "lightning-address", "challenge"],
            &[],
            Some(json!({"expected_revision":expected_revision})),
            Some(session.token()),
        )
        .await
    }
    pub async fn enroll_lightning_address(
        &self,
        session: &Session,
        challenge: &str,
        pubkey: &str,
        signature: &str,
    ) -> Result<LightningAddress, Error> {
        self.check_session(session)?;
        self.request(
            Method::PUT,
            &["device", "lightning-address"],
            &[],
            Some(json!({"challenge":challenge,"pubkey":pubkey,"signature":signature})),
            Some(session.token()),
        )
        .await
    }
    pub async fn disable_lightning_address(
        &self,
        session: &Session,
        name: &str,
        expected_revision: u64,
    ) -> Result<LightningAddress, Error> {
        self.check_session(session)?;
        self.request(
            Method::DELETE,
            &["device", "lightning-address"],
            &[],
            Some(json!({"name":name,"expected_revision":expected_revision})),
            Some(session.token()),
        )
        .await
    }
}
