//! Deployment-only native prepaid terms and secret references.
use serde::{Deserialize, Serialize};
use tenancy::money::funding::{FeePayer, Finality, Policy, Rounding, Unit};
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub merchant: String,
    pub live: bool,
    pub api_version: String,
    pub restricted_key_env: String,
    pub webhook_secret_envs: Vec<String>,
    pub policy: Policy,
    pub conversion: String,
    pub maximum_gross_units: u64,
    pub workspace_spend_limit_units: u64,
    pub checkout_seconds: u64,
    pub return_origin: String,
}
fn env_name(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 64
        && value.bytes().next().is_some_and(|b| b.is_ascii_uppercase())
        && value
            .bytes()
            .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit() || b == b'_')
}
impl Config {
    pub fn check(&self) -> Result<(), String> {
        super::identifier(&self.merchant, "acct_")?;
        self.policy.validate()?;
        let usd = Unit::CurrencyMillionths {
            currency: "USD".into(),
        };
        let conversion = self
            .policy
            .conversions
            .iter()
            .find(|c| c.version == self.conversion)
            .ok_or("The native card conversion is unavailable.")?;
        let origin = reqwest::Url::parse(&self.return_origin)
            .map_err(|_| "Invalid native checkout return origin.")?;
        if !env_name(&self.restricted_key_env)
            || self.webhook_secret_envs.is_empty()
            || self.webhook_secret_envs.len() > 2
            || self.webhook_secret_envs.iter().any(|v| !env_name(v))
            || self
                .webhook_secret_envs
                .iter()
                .collect::<std::collections::BTreeSet<_>>()
                .len()
                != self.webhook_secret_envs.len()
            || self.webhook_secret_envs.contains(&self.restricted_key_env)
            || self.api_version.is_empty()
            || self.api_version.len() > 64
            || !self
                .api_version
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'.')
            || self.policy.unit != usd
            || conversion.source != usd
            || conversion.target != usd
            || conversion.numerator != 1
            || conversion.denominator != 1
            || conversion.rounding != Rounding::Exact
            || conversion.fee_payer != FeePayer::Customer
            || self.policy.purchases.required_finality != Finality::Final
            || self.workspace_spend_limit_units == 0
            || self.maximum_gross_units % 10_000 != 0
            || !(50..=99_999_999).contains(&(self.maximum_gross_units / 10_000))
            || !(3600..=86_400).contains(&self.checkout_seconds)
            || self.return_origin.len() > 2048
            || origin.scheme() != "https"
            || origin.host_str().is_none()
            || !origin.username().is_empty()
            || origin.password().is_some()
            || !matches!(origin.path(), "" | "/")
            || origin.query().is_some()
            || origin.fragment().is_some()
            || origin.port().is_some_and(|p| p != 443)
        {
            return Err("Native prepaid cards require restricted secret references, exact USD terms, final collection, and an HTTPS return origin.".into());
        }
        Ok(())
    }
    pub fn digest(&self) -> String {
        receipts::execution::digest_request(
            &serde_json::to_value(self).expect("Native card configuration serializes."),
        )
    }
    pub async fn provider(&self) -> Result<super::Stripe, String> {
        self.check()?;
        let key = std::env::var(&self.restricted_key_env)
            .map_err(|_| "The restricted native card credential is unavailable.")?;
        let mut provider = super::Stripe::new_for_mode(key, self.api_version.clone(), self.live)?;
        provider.bind_account(&self.merchant).await?;
        Ok(provider)
    }
    pub fn webhook(&self, body: &[u8], signature: &str, now: u64) -> Result<super::Event, String> {
        self.check()?;
        for name in &self.webhook_secret_envs {
            let Ok(secret) = std::env::var(name) else {
                continue;
            };
            if let Ok(event) = super::verify_webhook(
                body,
                signature,
                secret.as_bytes(),
                now,
                300,
                &self.api_version,
                self.live,
            ) {
                return Ok(event);
            }
        }
        Err(super::refusal())
    }
}

#[cfg(test)]
pub(super) mod tests {
    use super::*;
    use tenancy::money::funding::{
        Conversion, POLICY_SCHEMA, PromotionTerms, PurchaseTerms, SpentCreditLoss,
    };

    pub(crate) fn config() -> Config {
        let usd = Unit::CurrencyMillionths {
            currency: "USD".into(),
        };
        Config {
            merchant: "acct_fixture".into(),
            live: false,
            api_version: "fixture.v1".into(),
            restricted_key_env: "OA_FIXTURE_CARD_KEY".into(),
            webhook_secret_envs: vec!["OA_FIXTURE_CARD_SIGNING".into()],
            conversion: "usd-exact-v1".into(),
            maximum_gross_units: 100_000_000,
            workspace_spend_limit_units: 1_000_000_000,
            checkout_seconds: 3600,
            return_origin: "https://fixture.invalid".into(),
            policy: Policy {
                schema: POLICY_SCHEMA.into(),
                version: "card-v1".into(),
                unit: usd.clone(),
                conversions: vec![Conversion {
                    version: "usd-exact-v1".into(),
                    source: usd.clone(),
                    target: usd,
                    numerator: 1,
                    denominator: 1,
                    rounding: Rounding::Exact,
                    fee_payer: FeePayer::Customer,
                    max_fee_units: 5_000_000,
                    source_ref: "fixture:no-real-money".into(),
                    valid_from: 0,
                    valid_until: u64::MAX,
                }],
                purchases: PurchaseTerms {
                    required_finality: Finality::Final,
                    refunds_allowed: true,
                    disputes_allowed: true,
                    spent_credit_loss: SpentCreditLoss::Operator,
                },
                promotions: PromotionTerms {
                    total_cap: 1,
                    grant_cap: 1,
                    max_lifetime_seconds: 1,
                    max_admissions: 1,
                    price_policies: ["fixture-use-v1".into()].into(),
                    reversible: true,
                },
            },
        }
    }
    #[test]
    fn configuration_requires_explicit_native_terms_and_secret_references() {
        let original = config();
        original.check().unwrap();
        let parsed: Config =
            serde_json::from_value(serde_json::to_value(&original).unwrap()).unwrap();
        assert_eq!(parsed.digest(), original.digest());
        let mut invalid = original.clone();
        invalid.workspace_spend_limit_units = 0;
        assert!(invalid.check().is_err());
        let mut invalid = original.clone();
        invalid.restricted_key_env = "rk_test_not_a_reference".into();
        assert!(invalid.check().is_err());
        let mut invalid = original.clone();
        invalid
            .webhook_secret_envs
            .push(invalid.webhook_secret_envs[0].clone());
        assert!(invalid.check().is_err());
        let mut invalid = original.clone();
        invalid.policy.conversions[0].numerator = 2;
        assert!(invalid.check().is_err());
        let mut invalid = original.clone();
        invalid.return_origin = "https://fixture.invalid/callback".into();
        assert!(invalid.check().is_err());
        let mut invalid = original.clone();
        invalid.checkout_seconds = 1800;
        assert!(invalid.check().is_err());
        let mut invalid = original.clone();
        invalid.policy.purchases.required_finality = Finality::Pending;
        assert!(invalid.check().is_err());
        let mut successor = original;
        successor.api_version = "fixture.v2".into();
        assert_ne!(successor.digest(), parsed.digest());
    }
}
