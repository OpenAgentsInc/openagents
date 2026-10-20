//! Account-owned Lightning addresses and permanent payment-name holds.
use super::{Accounts, Refusal, Store, Trouble, fresh, names};
use secp256k1::{Message, PublicKey, Secp256k1, ecdsa::Signature};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

pub const DOMAIN: &str = "openagents.com";
pub const CHALLENGE_TTL: u64 = 300;
const MAX_CHALLENGES: usize = 4;

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum State {
    Pending,
    Active,
    Disabled,
    Retired,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Address {
    pub name: String,
    pub account: String,
    pub nostr_pubkey: String,
    pub spark_pubkey: String,
    /// The wallet that must authorize a transfer before the replacement receives.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub previous_spark_pubkey: Option<String>,
    /// Earlier wallets retain signed access to their own payment metadata.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub historical_spark_pubkeys: Vec<String>,
    pub domain: String,
    pub revision: u64,
    pub state: State,
    pub created: u64,
    pub updated: u64,
    /// Identifies an already applied enrollment without keeping its signature.
    pub last_challenge: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Challenge {
    pub id: String,
    pub account: String,
    pub name: String,
    pub domain: String,
    pub operation: String,
    pub expected_revision: u64,
    pub expires_at: u64,
    pub message: String,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Book {
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub addresses: BTreeMap<String, Address>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub challenges: BTreeMap<String, Challenge>,
}
impl Book {
    pub fn is_empty(&self) -> bool {
        self.addresses.is_empty() && self.challenges.is_empty()
    }
    pub fn validate(&self) -> Result<(), String> {
        for (name, address) in &self.addresses {
            if name != &address.name
                || !names::valid_name(name)
                || address.domain != DOMAIN
                || address.revision == 0
                || address.account.is_empty()
                || address.last_challenge.len() != 64
                || address.updated < address.created
                || address.historical_spark_pubkeys.len() > 64
                || address
                    .historical_spark_pubkeys
                    .iter()
                    .any(|key| PublicKey::from_slice(&unhex(key).unwrap_or_default()).is_err())
                || address.previous_spark_pubkey.as_ref().is_some_and(|key| {
                    key == &address.spark_pubkey
                        || PublicKey::from_slice(&unhex(key).unwrap_or_default()).is_err()
                })
                || PublicKey::from_slice(&unhex(&address.spark_pubkey).unwrap_or_default()).is_err()
                || !super::nostr::valid_pubkey(&address.nostr_pubkey)
            {
                return Err("invalid Lightning address record".into());
            }
        }
        for (id, challenge) in &self.challenges {
            if id != &challenge.id
                || id.len() != 64
                || !id.bytes().all(|b| b.is_ascii_hexdigit())
                || challenge.account.is_empty()
                || !names::valid_name(&challenge.name)
                || challenge.operation != "enable"
                || challenge.domain != DOMAIN
                || challenge.message != statement(challenge)
            {
                return Err("invalid Lightning address challenge".into());
            }
        }
        Ok(())
    }
    /// A name once used for payments cannot be assigned to another account.
    pub fn held_by_other(&self, name: &str, account: &str) -> bool {
        self.addresses
            .get(name)
            .is_some_and(|record| record.account != account)
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Problem {
    NameRequired,
    Held,
    Conflict,
    InvalidProof,
    Expired,
    TooMany,
    NotFound,
}
impl Problem {
    pub fn message(self) -> &'static str {
        match self {
            Self::NameRequired => "Claim a name before enabling a Lightning address.",
            Self::Held => "That payment address belongs to another account.",
            Self::Conflict => "Your address changed. Refresh it and try again.",
            Self::InvalidProof => "The wallet signature couldn't be verified.",
            Self::Expired => "The request expired. Start again.",
            Self::TooMany => "A few requests are already waiting. Try again in a few minutes.",
            Self::NotFound => "That Lightning address isn't available.",
        }
    }
}

type Answer<T> = Result<Result<T, Problem>, Refusal>;

fn statement(challenge: &Challenge) -> String {
    // A fixed-order JSON tuple avoids ambiguous separators in signed fields.
    serde_json::to_string(&(
        "openagents.lightning-address.v1",
        &challenge.account,
        &challenge.name,
        &challenge.domain,
        &challenge.operation,
        challenge.expected_revision,
        &challenge.id,
        challenge.expires_at,
    ))
    .expect("the challenge serializes")
}
fn unhex(value: &str) -> Option<Vec<u8>> {
    if value.len() % 2 != 0 || value.len() > 256 {
        return None;
    }
    (0..value.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(value.get(i..i + 2)?, 16).ok())
        .collect()
}
fn wallet_key(message: &str, pubkey: &str, signature: &str) -> Option<String> {
    let key = PublicKey::from_slice(&unhex(pubkey)?).ok()?;
    let bytes = unhex(signature)?;
    let signature = Signature::from_der(&bytes)
        .or_else(|_| Signature::from_compact(&bytes))
        .ok()?;
    let digest: [u8; 32] = Sha256::digest(message.as_bytes()).into();
    Secp256k1::verification_only()
        .verify_ecdsa(Message::from_digest(digest), &signature, &key)
        .ok()?;
    Some(key.to_string())
}

/// An address may receive only while its original account still holds its profile key.
pub fn active(store: &Store, name: &str) -> Option<Address> {
    let address = store.lightning.addresses.get(name)?;
    let account = store.accounts.get(&address.account)?;
    (address.state == State::Active
        && super::nostr::keys_of(account).contains(&address.nostr_pubkey))
    .then(|| address.clone())
}

impl Accounts {
    pub fn lightning_address(&self, account: &str) -> Result<Vec<Address>, Trouble> {
        Ok(self
            .store()?
            .lightning
            .addresses
            .into_values()
            .filter(|a| a.account == account)
            .collect())
    }

    /// Issue a wallet proof scoped to the account's currently claimed name.
    pub fn lightning_challenge(&self, account: &str, expected_revision: u64) -> Answer<Challenge> {
        self.mutate(|store, now| {
            let Some((name, _)) = names::name_of(store, account) else {
                return Ok(Err(Problem::NameRequired));
            };
            if store.lightning.held_by_other(&name, account) {
                return Ok(Err(Problem::Held));
            }
            let current = store
                .lightning
                .addresses
                .get(&name)
                .map_or(0, |a| a.revision);
            if current != expected_revision {
                return Ok(Err(Problem::Conflict));
            }
            store.lightning.challenges.retain(|_, c| c.expires_at > now);
            if store
                .lightning
                .challenges
                .values()
                .filter(|c| c.account == account)
                .count()
                >= MAX_CHALLENGES
            {
                return Ok(Err(Problem::TooMany));
            }
            let mut challenge = Challenge {
                id: fresh().map_err(Refusal::Store)?,
                account: account.into(),
                name,
                domain: DOMAIN.into(),
                operation: "enable".into(),
                expected_revision,
                expires_at: now + CHALLENGE_TTL,
                message: String::new(),
            };
            challenge.message = statement(&challenge);
            store
                .lightning
                .challenges
                .insert(challenge.id.clone(), challenge.clone());
            Ok(Ok(challenge))
        })
    }

    /// Accept a local wallet signature and reserve the name in the same transaction.
    pub fn enroll_lightning(
        &self,
        account: &str,
        challenge_id: &str,
        pubkey: &str,
        signature: &str,
    ) -> Answer<Address> {
        self.mutate(|store, now| {
            let Some(challenge) = store.lightning.challenges.get(challenge_id).cloned() else {
                // Retrying exactly the enrollment that produced the current revision is safe.
                let applied = store
                    .lightning
                    .addresses
                    .values()
                    .find(|a| a.account == account && a.last_challenge == challenge_id);
                return Ok(match applied {
                    Some(a)
                        if a.spark_pubkey == pubkey
                            && matches!(a.state, State::Pending | State::Active) =>
                    {
                        Ok(a.clone())
                    }
                    _ => Err(Problem::Expired),
                });
            };
            if challenge.account != account {
                return Ok(Err(Problem::InvalidProof));
            }
            if challenge.expires_at <= now {
                return Ok(Err(Problem::Expired));
            }
            let Some((name, claim)) = names::name_of(store, account) else {
                return Ok(Err(Problem::NameRequired));
            };
            if name != challenge.name {
                return Ok(Err(Problem::Conflict));
            }
            if store.lightning.held_by_other(&name, account) {
                return Ok(Err(Problem::Held));
            }
            let old = store.lightning.addresses.get(&name);
            if old.map_or(0, |a| a.revision) != challenge.expected_revision {
                return Ok(Err(Problem::Conflict));
            }
            let Some(key) = wallet_key(&challenge.message, pubkey, signature) else {
                return Ok(Err(Problem::InvalidProof));
            };
            if store.lightning.addresses.values().any(|a| {
                a.account != account
                    && (a.spark_pubkey == key || a.historical_spark_pubkeys.contains(&key))
            }) {
                return Ok(Err(Problem::Held));
            }
            let mut historical = old.map_or_else(Vec::new, |a| a.historical_spark_pubkeys.clone());
            if let Some(old) = old
                && old.spark_pubkey != key
                && !historical.contains(&old.spark_pubkey)
            {
                if historical.len() >= 64 {
                    return Ok(Err(Problem::TooMany));
                }
                historical.push(old.spark_pubkey.clone());
            }
            let address = Address {
                name: name.clone(),
                account: account.into(),
                nostr_pubkey: claim.pubkey,
                previous_spark_pubkey: old.and_then(|a| {
                    let authority = a.previous_spark_pubkey.as_ref().unwrap_or(&a.spark_pubkey);
                    (authority != &key).then(|| authority.clone())
                }),
                historical_spark_pubkeys: historical,
                spark_pubkey: key,
                domain: DOMAIN.into(),
                revision: challenge.expected_revision + 1,
                state: State::Pending,
                created: old.map_or(now, |a| a.created),
                updated: now,
                last_challenge: challenge.id,
            };
            // Breez keeps one name per wallet and domain. Moving that wallet
            // stops its older address without releasing the payment-name hold.
            for previous in store.lightning.addresses.values_mut() {
                if previous.name != name
                    && previous.spark_pubkey == address.spark_pubkey
                    && previous.account == account
                    && matches!(previous.state, State::Pending | State::Active)
                {
                    previous.state = State::Disabled;
                    previous.revision += 1;
                    previous.updated = now;
                }
            }
            store.lightning.addresses.insert(name, address.clone());
            store.lightning.challenges.remove(challenge_id);
            Ok(Ok(address))
        })
    }

    /// Confirm the exact wallet registration that the private backend accepted.
    pub fn activate_lightning(&self, name: &str, key: &str, revision: u64) -> Answer<Address> {
        self.mutate(|store, now| {
            let Some(address) = store.lightning.addresses.get_mut(name) else {
                return Ok(Err(Problem::NotFound));
            };
            if address.spark_pubkey != key
                || address.revision != revision
                || !matches!(address.state, State::Pending | State::Active)
            {
                return Ok(Err(Problem::Conflict));
            }
            address.previous_spark_pubkey = None;
            address.state = State::Active;
            address.updated = now;
            Ok(Ok(address.clone()))
        })
    }

    pub fn disable_lightning(&self, account: &str, name: &str, revision: u64) -> Answer<Address> {
        self.mutate(|store, now| {
            let Some(address) = store.lightning.addresses.get_mut(name) else {
                return Ok(Err(Problem::NotFound));
            };
            if address.account != account {
                return Ok(Err(Problem::Held));
            }
            if address.revision != revision {
                return Ok(Err(Problem::Conflict));
            }
            address.state = State::Disabled;
            address.revision += 1;
            address.updated = now;
            Ok(Ok(address.clone()))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::super::names::{Claimed, NameProblem};
    use super::*;
    fn fixture() -> (tempfile::TempDir, Accounts, String, String) {
        let dir = tempfile::tempdir().unwrap();
        let accounts = Accounts::install(dir.path()).unwrap();
        let a = accounts
            .sign_in_nostr(&"11".repeat(32), "signup")
            .unwrap()
            .account
            .id;
        let b = accounts
            .sign_in_nostr(&"22".repeat(32), "signup")
            .unwrap()
            .account
            .id;
        assert!(matches!(
            accounts.claim_name(&a, "lightning-user", None).unwrap(),
            Claimed::Done { .. }
        ));
        (dir, accounts, a, b)
    }
    fn proof(challenge: &Challenge) -> (String, String) {
        let secret = secp256k1::SecretKey::from_byte_array([7; 32]).unwrap();
        let secp = Secp256k1::new();
        let key = PublicKey::from_secret_key(&secp, &secret).to_string();
        let digest: [u8; 32] = Sha256::digest(challenge.message.as_bytes()).into();
        let signature = secp.sign_ecdsa(Message::from_digest(digest), &secret);
        (
            key,
            signature
                .serialize_der()
                .iter()
                .map(|b| format!("{b:02x}"))
                .collect(),
        )
    }
    #[test]
    fn enabled_addresses_keep_their_owner_after_release_and_reload() {
        let (dir, accounts, a, b) = fixture();
        let challenge = accounts.lightning_challenge(&a, 0).unwrap().unwrap();
        let (key, signature) = proof(&challenge);
        let record = accounts
            .enroll_lightning(&a, &challenge.id, &key, &signature)
            .unwrap()
            .unwrap();
        assert_eq!(record.state, State::Pending);
        assert!(active(&accounts.store().unwrap(), &record.name).is_none());
        accounts
            .activate_lightning(&record.name, &key, record.revision)
            .unwrap()
            .unwrap();
        assert!(active(&accounts.store().unwrap(), &record.name).is_some());
        accounts.release_name(&a).unwrap();
        let reopened = Accounts::open(dir.path()).unwrap();
        assert!(matches!(
            reopened.claim_name(&b, &record.name, None).unwrap(),
            Claimed::Refused(NameProblem::Taken)
        ));
        let disabled = reopened
            .disable_lightning(&a, &record.name, record.revision)
            .unwrap()
            .unwrap();
        assert_eq!(disabled.state, State::Disabled);
        assert!(active(&reopened.store().unwrap(), &record.name).is_none());
        assert!(matches!(
            reopened.claim_name(&b, &record.name, None).unwrap(),
            Claimed::Refused(NameProblem::Taken)
        ));
    }
    #[test]
    fn proofs_bind_account_statement_and_revision_and_retry_once() {
        let (_dir, accounts, a, b) = fixture();
        let challenge = accounts.lightning_challenge(&a, 0).unwrap().unwrap();
        let (key, signature) = proof(&challenge);
        assert_eq!(
            accounts
                .enroll_lightning(&b, &challenge.id, &key, &signature)
                .unwrap(),
            Err(Problem::InvalidProof)
        );
        let mut changed = challenge.clone();
        changed.message.push('x');
        let (_, bad) = proof(&changed);
        assert_eq!(
            accounts
                .enroll_lightning(&a, &challenge.id, &key, &bad)
                .unwrap(),
            Err(Problem::InvalidProof)
        );
        let first = accounts
            .enroll_lightning(&a, &challenge.id, &key, &signature)
            .unwrap()
            .unwrap();
        assert_eq!(
            accounts
                .enroll_lightning(&a, &challenge.id, &key, &signature)
                .unwrap()
                .unwrap(),
            first
        );
        assert_eq!(
            accounts.lightning_challenge(&a, 0).unwrap(),
            Err(Problem::Conflict)
        );
        assert_eq!(
            accounts
                .disable_lightning(&b, &first.name, first.revision)
                .unwrap(),
            Err(Problem::Held)
        );
        accounts
            .disable_lightning(&a, &first.name, first.revision)
            .unwrap()
            .unwrap();
        assert_eq!(
            accounts
                .activate_lightning(&first.name, &key, first.revision)
                .unwrap(),
            Err(Problem::Conflict)
        );
        assert_eq!(
            accounts
                .enroll_lightning(&a, &challenge.id, &key, &signature)
                .unwrap(),
            Err(Problem::Expired)
        );
    }
    #[test]
    fn removing_the_identity_stops_receiving_without_releasing_the_name() {
        let (_dir, accounts, a, b) = fixture();
        let challenge = accounts.lightning_challenge(&a, 0).unwrap().unwrap();
        let (key, signature) = proof(&challenge);
        let record = accounts
            .enroll_lightning(&a, &challenge.id, &key, &signature)
            .unwrap()
            .unwrap();
        accounts
            .activate_lightning(&record.name, &key, record.revision)
            .unwrap()
            .unwrap();
        accounts.delete_account(&a).unwrap();
        let snapshot = accounts.store().unwrap();
        assert!(active(&snapshot, &record.name).is_none());
        assert!(snapshot.lightning.held_by_other(&record.name, &b));
        assert!(matches!(
            accounts.claim_name(&b, &record.name, None).unwrap(),
            Claimed::Refused(NameProblem::Taken)
        ));
    }

    #[test]
    fn a_wallet_rename_disables_the_old_address_and_keeps_its_hold() {
        let (_dir, accounts, a, b) = fixture();
        let challenge = accounts.lightning_challenge(&a, 0).unwrap().unwrap();
        let (key, signature) = proof(&challenge);
        let old = accounts
            .enroll_lightning(&a, &challenge.id, &key, &signature)
            .unwrap()
            .unwrap();
        accounts
            .activate_lightning(&old.name, &key, old.revision)
            .unwrap()
            .unwrap();
        assert!(matches!(
            accounts.claim_name(&a, "new-payment-name", None).unwrap(),
            Claimed::Done { .. }
        ));
        let challenge = accounts.lightning_challenge(&a, 0).unwrap().unwrap();
        let (key, signature) = proof(&challenge);
        let new = accounts
            .enroll_lightning(&a, &challenge.id, &key, &signature)
            .unwrap()
            .unwrap();
        let store = accounts.store().unwrap();
        assert_eq!(store.lightning.addresses[&old.name].state, State::Disabled);
        assert!(store.lightning.held_by_other(&old.name, &b));
        assert_eq!(new.name, "new-payment-name");
        assert!(new.previous_spark_pubkey.is_none());
    }

    #[test]
    fn replacing_a_wallet_keeps_the_old_wallet_as_required_transfer_authority() {
        let (_dir, accounts, a, _) = fixture();
        let challenge = accounts.lightning_challenge(&a, 0).unwrap().unwrap();
        let (old, signature) = proof(&challenge);
        let record = accounts
            .enroll_lightning(&a, &challenge.id, &old, &signature)
            .unwrap()
            .unwrap();
        accounts
            .activate_lightning(&record.name, &old, record.revision)
            .unwrap()
            .unwrap();
        let challenge = accounts
            .lightning_challenge(&a, record.revision)
            .unwrap()
            .unwrap();
        let secret = secp256k1::SecretKey::from_byte_array([8; 32]).unwrap();
        let secp = Secp256k1::new();
        let key = PublicKey::from_secret_key(&secp, &secret).to_string();
        let digest: [u8; 32] = Sha256::digest(challenge.message.as_bytes()).into();
        let signature = secp
            .sign_ecdsa(Message::from_digest(digest), &secret)
            .serialize_der()
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>();
        let replacement = accounts
            .enroll_lightning(&a, &challenge.id, &key, &signature)
            .unwrap()
            .unwrap();
        assert!(replacement.historical_spark_pubkeys.contains(&old));
        assert_eq!(replacement.previous_spark_pubkey, Some(old));
        assert_eq!(replacement.state, State::Pending);
        assert!(active(&accounts.store().unwrap(), &record.name).is_none());
        assert_eq!(
            accounts
                .activate_lightning(&record.name, &key, record.revision)
                .unwrap(),
            Err(Problem::Conflict)
        );
    }

    #[test]
    fn the_original_wallet_can_cancel_an_unfinished_rotation() {
        let (_dir, accounts, a, _) = fixture();
        let first = accounts.lightning_challenge(&a, 0).unwrap().unwrap();
        let (key, signature) = proof(&first);
        let record = accounts
            .enroll_lightning(&a, &first.id, &key, &signature)
            .unwrap()
            .unwrap();
        accounts
            .mutate(|store, _| {
                let record = store.lightning.addresses.get_mut(&record.name).unwrap();
                record.previous_spark_pubkey = Some(key.clone());
                record.spark_pubkey = PublicKey::from_secret_key(
                    &Secp256k1::new(),
                    &secp256k1::SecretKey::from_byte_array([8; 32]).unwrap(),
                )
                .to_string();
                Ok(())
            })
            .unwrap();
        let retry = accounts
            .lightning_challenge(&a, record.revision)
            .unwrap()
            .unwrap();
        let (key, signature) = proof(&retry);
        let restored = accounts
            .enroll_lightning(&a, &retry.id, &key, &signature)
            .unwrap()
            .unwrap();
        assert!(restored.previous_spark_pubkey.is_none());
        assert_eq!(restored.spark_pubkey, key);
    }

    #[test]
    fn expiry_and_pending_challenge_bounds_are_enforced() {
        let (_dir, accounts, a, _) = fixture();
        let challenge = accounts.lightning_challenge(&a, 0).unwrap().unwrap();
        let (key, signature) = proof(&challenge);
        for _ in 1..MAX_CHALLENGES {
            accounts.lightning_challenge(&a, 0).unwrap().unwrap();
        }
        assert_eq!(
            accounts.lightning_challenge(&a, 0).unwrap(),
            Err(Problem::TooMany)
        );
        accounts
            .mutate(|store, _| {
                let c = store.lightning.challenges.get_mut(&challenge.id).unwrap();
                c.expires_at = 1;
                c.message = statement(c);
                Ok(())
            })
            .unwrap();
        assert_eq!(
            accounts
                .enroll_lightning(&a, &challenge.id, &key, &signature)
                .unwrap(),
            Err(Problem::Expired)
        );
    }
}
