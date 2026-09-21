//! The key store: who a credential belongs to, and whether it stands.
//!
//! A key is the shape `oak_<id>.<secret>` — the id is a locator, the
//! secret is the credential, and the store keeps only the secret's
//! SHA-256 digest. Nothing here ever writes the secret back out:
//! `issue` returns it once, on the response the operator reads, and after
//! that the secret exists only where the caller keeps it. Logs, history,
//! and the store itself see the id and the digest.
//!
//! Provisioning is an operator path, not a self-serve one: `issue`,
//! `rotate`, and `revoke` write a `keys.json` beside `registry.json`, and
//! authentication is a read of that store plus a lookup in the manifest.
//! Rotation binds a new key to the same tenant — a rotated tenant's quota
//! and doors are untouched, because the tenant record never moved.

use std::collections::BTreeMap;
use std::path::Path;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::manifest::Manifest;

/// The file the key store lives in, beside `registry.json`.
const KEYS: &str = "keys.json";

/// The schema tag the store carries.
pub const KEYS_SCHEMA: &str = "openagents.tenancy.keys.v1";

/// The token's wire prefix, so a pasted credential announces its shape.
const PREFIX: &str = "oak";

/// One key's record. The secret is never here — only its digest.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Key {
    /// The key's id — the token's middle segment.
    pub id: String,
    /// The tenant the key belongs to.
    pub tenant: String,
    /// SHA-256 of the secret, hex.
    pub digest: String,
    /// `active` or `revoked`.
    pub status: Status,
    /// When the key was issued, as RFC 3339 in UTC.
    pub created: String,
    /// The key id this one replaced, when it is a rotation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rotated_from: Option<String>,
}

/// Whether a key stands.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Status {
    /// Authenticates.
    Active,
    /// Refused on sight.
    Revoked,
}

/// The store document: a versioned map from key id to key record.
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
pub struct KeyStore {
    /// The schema tag.
    #[serde(rename = "v", default)]
    pub v: String,
    /// Key id to record.
    #[serde(default)]
    pub keys: BTreeMap<String, Key>,
}

/// What a successful authentication names.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Authenticated {
    /// The tenant the key belongs to.
    pub tenant: String,
    /// The key that authenticated — per-key accounting, revocation, and
    /// rotation all need the id, not just the tenant.
    pub key_id: String,
}

/// Why a presented credential was refused.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AuthRefusal {
    /// No credential was presented.
    Missing,
    /// The credential is not `oak_<id>.<secret>` shaped.
    Malformed,
    /// The id names no key the store holds.
    Unknown(String),
    /// The key exists and is revoked.
    Revoked(String),
    /// The secret does not match the stored digest.
    WrongSecret(String),
    /// The key is valid but its tenant is no longer in the registry —
    /// authentication succeeded and authorization still refuses.
    TenantGone { tenant: String, key: String },
}

impl std::fmt::Display for AuthRefusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Missing => write!(f, "no credential was presented"),
            Self::Malformed => {
                write!(f, "the credential is not an `{PREFIX}_<id>.<secret>` key")
            }
            Self::Unknown(id) => write!(f, "key `{id}` is not one this store issued"),
            Self::Revoked(id) => write!(f, "key `{id}` is revoked"),
            Self::WrongSecret(id) => {
                write!(f, "key `{id}`'s secret does not match")
            }
            Self::TenantGone { tenant, key } => write!(
                f,
                "key `{key}` is valid but tenant `{tenant}` is not in the registry"
            ),
        }
    }
}

/// What went wrong writing the store.
#[derive(Debug)]
pub enum KeyTrouble {
    /// The filesystem refused.
    Io(std::io::Error),
    /// The store document did not parse or failed its own checks.
    Invalid(String),
    /// An issue or rotation named a tenant the manifest does not hold.
    UnknownTenant(String),
    /// A rotation or revocation named a key the store does not hold.
    UnknownKey(String),
}

impl std::fmt::Display for KeyTrouble {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(error) => write!(f, "{error}"),
            Self::Invalid(message) => write!(f, "{message}"),
            Self::UnknownTenant(tenant) => write!(
                f,
                "tenant `{tenant}` is not in the registry — bind the tenant \
                 before issuing it a key"
            ),
            Self::UnknownKey(id) => write!(f, "key `{id}` is not one this store holds"),
        }
    }
}

impl std::error::Error for KeyTrouble {}

impl From<std::io::Error> for KeyTrouble {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error)
    }
}

/// What `issue` and `rotate` hand back: the record and, once, the secret.
pub struct Issued {
    /// The stored record — safe to log and keep.
    pub key: Key,
    /// The full token, `oak_<id>.<secret>`. This is the only place the
    /// secret exists outside the caller's hands; the store cannot
    /// reproduce it.
    pub token: String,
}

/// Parse a presented token into its id and secret halves.
fn split(token: &str) -> Option<(String, String)> {
    let body = token.strip_prefix(&format!("{PREFIX}_"))?;
    let (id, secret) = body.split_once('.')?;
    if id.is_empty() || secret.len() != 64 || !secret.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    Some((id.to_string(), secret.to_string()))
}

/// The digest the store keeps of a secret.
fn digest_secret(secret: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(secret.as_bytes());
    format!("{:x}", hasher.finalize())
}

/// Fresh random material for ids and secrets — 32 bytes of hex.
fn fresh() -> Result<String, KeyTrouble> {
    let mut bytes = [0_u8; 32];
    getrandom::fill(&mut bytes)
        .map_err(|error| KeyTrouble::Invalid(format!("no randomness available: {error}")))?;
    Ok(bytes.iter().map(|byte| format!("{byte:02x}")).collect())
}

/// Load the key store from a registry directory.
pub fn load(dir: &Path) -> Result<KeyStore, KeyTrouble> {
    let path = dir.join(KEYS);
    match std::fs::read_to_string(&path) {
        Ok(text) => {
            let store: KeyStore = serde_json::from_str(&text)
                .map_err(|error| KeyTrouble::Invalid(format!("{}: {error}", path.display())))?;
            if store.v != KEYS_SCHEMA {
                return Err(KeyTrouble::Invalid(format!(
                    "{}: schema `{}` is not `{KEYS_SCHEMA}`",
                    path.display(),
                    store.v
                )));
            }
            Ok(store)
        }
        // No store is an empty store — a registry that never issued a key
        // authenticates nothing and refuses cleanly.
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(KeyStore {
            v: KEYS_SCHEMA.to_string(),
            keys: BTreeMap::new(),
        }),
        Err(error) => Err(KeyTrouble::Io(error)),
    }
}

/// Write the store atomically: a temp file and a rename, the same way the
/// manifest lands.
fn save(dir: &Path, store: &KeyStore) -> Result<(), KeyTrouble> {
    let text = serde_json::to_string_pretty(store)
        .map_err(|error| KeyTrouble::Invalid(error.to_string()))?;
    let staged = dir.join(format!(".{KEYS}.tmp"));
    std::fs::write(&staged, format!("{text}\n"))?;
    std::fs::rename(&staged, dir.join(KEYS))?;
    Ok(())
}

/// Issue a key for a tenant the registry binds.
pub fn issue(dir: &Path, manifest: &Manifest, tenant: &str) -> Result<Issued, KeyTrouble> {
    if !manifest.tenants.contains_key(tenant) {
        return Err(KeyTrouble::UnknownTenant(tenant.to_string()));
    }
    let mut store = load(dir)?;
    let id = &fresh()?[..16];
    let secret = fresh()?;
    let key = Key {
        id: id.to_string(),
        tenant: tenant.to_string(),
        digest: digest_secret(&secret),
        status: Status::Active,
        created: crate::registry::now_utc(),
        rotated_from: None,
    };
    store.keys.insert(id.to_string(), key.clone());
    save(dir, &store)?;
    Ok(Issued {
        key,
        token: format!("{PREFIX}_{id}.{secret}"),
    })
}

/// Rotate a key: issue a fresh one for the same tenant and revoke the old.
///
/// Rotation keeps the tenant — the manifest never moves, so the tenant's
/// doors and quota carry straight across — and names the superseded key
/// in `rotated_from` so the lineage is auditable.
pub fn rotate(dir: &Path, key_id: &str) -> Result<Issued, KeyTrouble> {
    let mut store = load(dir)?;
    let old = store
        .keys
        .get(key_id)
        .ok_or_else(|| KeyTrouble::UnknownKey(key_id.to_string()))?
        .clone();
    let id = &fresh()?[..16];
    let secret = fresh()?;
    let key = Key {
        id: id.to_string(),
        tenant: old.tenant.clone(),
        digest: digest_secret(&secret),
        status: Status::Active,
        created: crate::registry::now_utc(),
        rotated_from: Some(old.id.clone()),
    };
    store.keys.insert(id.to_string(), key.clone());
    store.keys.get_mut(key_id).unwrap().status = Status::Revoked;
    save(dir, &store)?;
    Ok(Issued {
        key,
        token: format!("{PREFIX}_{id}.{secret}"),
    })
}

/// Revoke a key. The record stays — a revoked key must still be
/// recognizable as revoked rather than unknown.
pub fn revoke(dir: &Path, key_id: &str) -> Result<(), KeyTrouble> {
    let mut store = load(dir)?;
    let key = store
        .keys
        .get_mut(key_id)
        .ok_or_else(|| KeyTrouble::UnknownKey(key_id.to_string()))?;
    key.status = Status::Revoked;
    save(dir, &store)
}

/// Resolve a presented token to its tenant.
///
/// This is the only read the hot path needs: parse, look up, compare the
/// digest, check the status, confirm the tenant is still bound. Every
/// refusal is a distinct variant so a log line can name the reason
/// without naming the secret.
pub fn authenticate(
    dir: &Path,
    manifest: &Manifest,
    token: &str,
) -> Result<Authenticated, AuthRefusal> {
    let (id, secret) = split(token).ok_or(AuthRefusal::Malformed)?;
    let store = load(dir).map_err(|_| AuthRefusal::Malformed)?;
    let key = store
        .keys
        .get(&id)
        .ok_or_else(|| AuthRefusal::Unknown(id.clone()))?;
    if key.status == Status::Revoked {
        return Err(AuthRefusal::Revoked(id));
    }
    if key.digest != digest_secret(&secret) {
        return Err(AuthRefusal::WrongSecret(id));
    }
    if !manifest.tenants.contains_key(&key.tenant) {
        return Err(AuthRefusal::TenantGone {
            tenant: key.tenant.clone(),
            key: id,
        });
    }
    Ok(Authenticated {
        tenant: key.tenant.clone(),
        key_id: key.id.clone(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::manifest::Tenant;
    use crate::registry::Registry;

    fn manifest() -> Manifest {
        let mut tenants = BTreeMap::new();
        tenants.insert(
            "acme".to_string(),
            Tenant {
                credential: "key-ref:acme".to_string(),
                principals: vec![],
                doors: BTreeMap::new(),
                quota: None,
            },
        );
        tenants.insert(
            "globex".to_string(),
            Tenant {
                credential: "key-ref:globex".to_string(),
                principals: vec![],
                doors: BTreeMap::new(),
                quota: None,
            },
        );
        Manifest {
            v: crate::SCHEMA.to_string(),
            sequence: 0,
            supersedes: None,
            shared: BTreeMap::new(),
            tenants,
            digest: String::new(),
        }
    }

    fn installed() -> (tempfile::TempDir, Manifest) {
        let dir = tempfile::tempdir().unwrap();
        let registry = Registry::install(dir.path(), manifest()).unwrap();
        (dir, registry.manifest().clone())
    }

    #[test]
    fn issue_authenticates_and_never_stores_the_secret() {
        let (dir, manifest) = installed();
        let issued = issue(dir.path(), &manifest, "acme").unwrap();
        let auth = authenticate(dir.path(), &manifest, &issued.token).unwrap();
        assert_eq!(auth.tenant, "acme");
        assert_eq!(auth.key_id, issued.key.id);
        // The store on disk carries the id and digest, not the secret.
        let text = std::fs::read_to_string(dir.path().join(KEYS)).unwrap();
        assert!(!text.contains(&issued.token[5..]), "{text}");
        assert!(text.contains(&issued.key.digest));
    }

    #[test]
    fn two_keys_for_two_tenants_stay_independent() {
        let (dir, manifest) = installed();
        let acme = issue(dir.path(), &manifest, "acme").unwrap();
        let globex = issue(dir.path(), &manifest, "globex").unwrap();
        assert_eq!(
            authenticate(dir.path(), &manifest, &acme.token)
                .unwrap()
                .tenant,
            "acme"
        );
        assert_eq!(
            authenticate(dir.path(), &manifest, &globex.token)
                .unwrap()
                .tenant,
            "globex"
        );
        assert_ne!(acme.key.id, globex.key.id);
    }

    #[test]
    fn missing_malformed_unknown_and_wrong_are_distinct_refusals() {
        let (dir, manifest) = installed();
        let issued = issue(dir.path(), &manifest, "acme").unwrap();
        let id = &issued.key.id;
        assert_eq!(
            authenticate(dir.path(), &manifest, ""),
            Err(AuthRefusal::Malformed)
        );
        assert_eq!(
            authenticate(dir.path(), &manifest, "not-a-key"),
            Err(AuthRefusal::Malformed)
        );
        assert_eq!(
            authenticate(
                dir.path(),
                &manifest,
                &format!("{PREFIX}_nosuchkey.{digest}", digest = "0".repeat(64))
            ),
            Err(AuthRefusal::Unknown("nosuchkey".to_string()))
        );
        let wrong = format!("{PREFIX}_{id}.{}", "f".repeat(64));
        assert_eq!(
            authenticate(dir.path(), &manifest, &wrong),
            Err(AuthRefusal::WrongSecret(id.clone()))
        );
    }

    #[test]
    fn a_revoked_key_refuses_and_stays_recognizable() {
        let (dir, manifest) = installed();
        let issued = issue(dir.path(), &manifest, "acme").unwrap();
        revoke(dir.path(), &issued.key.id).unwrap();
        assert_eq!(
            authenticate(dir.path(), &manifest, &issued.token),
            Err(AuthRefusal::Revoked(issued.key.id.clone()))
        );
        // The record persists — revoked is a state, not an absence.
        let store = load(dir.path()).unwrap();
        assert_eq!(store.keys[&issued.key.id].status, Status::Revoked);
    }

    #[test]
    fn rotation_keeps_the_tenant_and_retires_the_old_key() {
        let (dir, manifest) = installed();
        let issued = issue(dir.path(), &manifest, "acme").unwrap();
        let rotated = rotate(dir.path(), &issued.key.id).unwrap();
        assert_eq!(rotated.key.tenant, "acme");
        assert_eq!(
            rotated.key.rotated_from.as_deref(),
            Some(issued.key.id.as_str())
        );
        // Old key is dead; new key authenticates as the same tenant.
        assert_eq!(
            authenticate(dir.path(), &manifest, &issued.token),
            Err(AuthRefusal::Revoked(issued.key.id.clone()))
        );
        assert_eq!(
            authenticate(dir.path(), &manifest, &rotated.token)
                .unwrap()
                .tenant,
            "acme"
        );
        // The registry never moved — the digest is untouched.
        let registry = Registry::open(dir.path()).unwrap();
        assert_eq!(registry.digest(), manifest.digest);
    }

    #[test]
    fn a_key_for_an_unknown_tenant_is_refused_at_issue() {
        let (dir, manifest) = installed();
        assert!(matches!(
            issue(dir.path(), &manifest, "initech"),
            Err(KeyTrouble::UnknownTenant(_))
        ));
    }

    #[test]
    fn a_tenant_removed_from_the_registry_strands_its_keys() {
        let (dir, manifest) = installed();
        let issued = issue(dir.path(), &manifest, "acme").unwrap();
        let mut gone = manifest.clone();
        gone.tenants.remove("acme");
        gone.sequence = 1;
        gone.supersedes = Some(manifest.digest.clone());
        Registry::update(dir.path(), gone).unwrap();
        let opened = Registry::open(dir.path()).unwrap();
        assert_eq!(
            authenticate(dir.path(), opened.manifest(), &issued.token),
            Err(AuthRefusal::TenantGone {
                tenant: "acme".to_string(),
                key: issued.key.id.clone()
            })
        );
    }
}
