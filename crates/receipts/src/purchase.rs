//! Frozen customer and price attribution for an explicit decision purchase.
//! These records narrow an authenticated call; they never grant authority.

use crate::execution::digest_request;
use serde::{Deserialize, Serialize};

pub const SCHEMA: &str = "openagents.purchase-context.v1";
pub const MAX_QUOTE_MS: u64 = 300_000;
pub const HEADER: &str = "x-openagents-purchase";

/// References to the existing gateway price and its bounded reservation.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PriceReference {
    pub version: String,
    pub currency: String,
    pub policy: String,
    pub terms_digest: String,
    pub maximum_usage_digest: String,
    pub maximum_charge: u64,
}

/// The current authenticated account, membership, resource, and payer.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Context {
    pub schema: String,
    pub account: String,
    pub workspace: String,
    pub payer_workspace: String,
    pub tenant: String,
    pub credential_reference: String,
    pub membership_epoch: u64,
    pub workspace_members_epoch: u64,
    pub role: String,
    pub door: String,
    pub registry_digest: String,
    pub artifact_digest: String,
    pub price: PriceReference,
    pub can_invoke: bool,
}

fn identity(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"-_.:".contains(&b))
}
fn hash(value: &str) -> bool {
    let Some(value) = value.strip_prefix("sha256:") else {
        return false;
    };
    value.len() == 64
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
fn digest<T: Serialize>(value: &T) -> String {
    digest_request(&serde_json::to_value(value).expect("purchase DTO serializes"))
}
impl Context {
    pub fn validate(&self) -> Result<(), &'static str> {
        if self.schema != SCHEMA
            || [
                &self.account,
                &self.workspace,
                &self.payer_workspace,
                &self.tenant,
                &self.credential_reference,
                &self.door,
                &self.price.version,
                &self.price.currency,
                &self.price.policy,
            ]
            .into_iter()
            .any(|v| !identity(v))
            || self.workspace != self.payer_workspace
            || !matches!(self.role.as_str(), "owner" | "admin" | "member")
            || [
                &self.registry_digest,
                &self.artifact_digest,
                &self.price.terms_digest,
                &self.price.maximum_usage_digest,
            ]
            .into_iter()
            .any(|v| !hash(v))
        {
            return Err("Invalid customer purchase context.");
        }
        Ok(())
    }
    pub fn digest(&self) -> String {
        digest(self)
    }
}

/// One exact request under frozen customer and price references.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Quote {
    pub id: String,
    pub context: Context,
    pub request_digest: String,
    pub created_at_ms: u64,
    pub expires_at_ms: u64,
}
impl Quote {
    pub fn validate(&self) -> Result<(), &'static str> {
        self.context.validate()?;
        if !identity(&self.id)
            || !hash(&self.request_digest)
            || !self.context.can_invoke
            || self.expires_at_ms <= self.created_at_ms
            || self.expires_at_ms - self.created_at_ms > MAX_QUOTE_MS
        {
            return Err("Invalid or unavailable purchase quote.");
        }
        Ok(())
    }
    pub fn digest(&self) -> String {
        digest(self)
    }
}

/// Explicit approval of a quote; bearer and current server rights still apply.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Approval {
    pub quote: Quote,
    pub approved_at_ms: u64,
}
impl Approval {
    pub fn validate_current(
        &self,
        current: &Context,
        request_digest: &str,
        now_ms: u64,
    ) -> Result<(), &'static str> {
        self.quote.validate()?;
        current.validate()?;
        if !current.can_invoke
            || current != &self.quote.context
            || request_digest != self.quote.request_digest
            || self.approved_at_ms < self.quote.created_at_ms
            || self.approved_at_ms > now_ms
            || now_ms >= self.quote.expires_at_ms
        {
            return Err(
                "Purchase approval is expired or its customer, rights, price, or request changed.",
            );
        }
        Ok(())
    }
    pub fn digest(&self) -> String {
        digest(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn approval() -> Approval {
        let context = Context {
            schema: SCHEMA.into(),
            account: "buyer-a".into(),
            workspace: "workspace-a".into(),
            payer_workspace: "workspace-a".into(),
            tenant: "tenant-a".into(),
            credential_reference: "key-a".into(),
            membership_epoch: 1,
            workspace_members_epoch: 1,
            role: "owner".into(),
            door: "decision-a".into(),
            registry_digest: format!("sha256:{}", "a".repeat(64)),
            artifact_digest: format!("sha256:{}", "b".repeat(64)),
            price: PriceReference {
                version: "price-1".into(),
                currency: "USD".into(),
                policy: "observed-usage-v1".into(),
                terms_digest: format!("sha256:{}", "c".repeat(64)),
                maximum_usage_digest: format!("sha256:{}", "d".repeat(64)),
                maximum_charge: 100,
            },
            can_invoke: true,
        };
        Approval {
            quote: Quote {
                id: "purchase-1".into(),
                context,
                request_digest: format!("sha256:{}", "e".repeat(64)),
                created_at_ms: 100,
                expires_at_ms: 200,
            },
            approved_at_ms: 110,
        }
    }
    #[test]
    fn approval_cannot_follow_account_workspace_credential_price_or_rights_changes() {
        let a = approval();
        let current = a.quote.context.clone();
        assert!(
            a.validate_current(&current, &a.quote.request_digest, 120)
                .is_ok()
        );
        let mut variants = Vec::new();
        let mut c = current.clone();
        c.account = "buyer-b".into();
        variants.push(c);
        let mut c = current.clone();
        c.workspace = "workspace-b".into();
        c.payer_workspace = "workspace-b".into();
        variants.push(c);
        let mut c = current.clone();
        c.credential_reference = "key-rotated".into();
        variants.push(c);
        let mut c = current.clone();
        c.membership_epoch += 1;
        variants.push(c);
        let mut c = current.clone();
        c.price.maximum_charge += 1;
        variants.push(c);
        let mut c = current.clone();
        c.price.terms_digest = format!("sha256:{}", "f".repeat(64));
        variants.push(c);
        let mut c = current;
        c.can_invoke = false;
        variants.push(c);
        for c in variants {
            assert!(
                a.validate_current(&c, &a.quote.request_digest, 120)
                    .is_err()
            );
        }
    }
    #[test]
    fn exact_request_expiry_and_bounded_approval_are_required() {
        let a = approval();
        let c = &a.quote.context;
        assert!(
            a.validate_current(c, &format!("sha256:{}", "f".repeat(64)), 120)
                .is_err()
        );
        assert!(a.validate_current(c, &a.quote.request_digest, 200).is_err());
        assert!(a.validate_current(c, &a.quote.request_digest, 109).is_err());
        let mut a = a;
        a.quote.expires_at_ms = a.quote.created_at_ms + MAX_QUOTE_MS + 1;
        assert!(a.quote.validate().is_err());
    }
}
