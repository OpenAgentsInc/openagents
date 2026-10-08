//! Enterprise sign-in beside native account authority (REV-50).
//!
//! One workspace admits one OpenID Connect provider at a time: an issuer,
//! a tenant claim, an audience, and the signing keys the owner reviewed.
//! A verified token signs in only an account that an admin linked to its
//! subject, or, under the verified-domain rule, the one active member whose
//! label is the verified email. The book keeps digests and references;
//! it never keeps a token, a key secret, or a claim set.
use super::{Lock, MemberStatus, Refusal, Role, Store, active_member};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};

pub const SCHEMA: &str = "openagents.sso-provider.v1";
pub const MAX_SKEW_SECS: u64 = 300;
pub const MAX_TOKEN_LIFETIME_SECS: u64 = 3600;
pub const MAX_AUDIT: usize = 4096;

/// How a verified subject resolves to an account.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Linking {
    /// Only an admin-linked subject signs in.
    LinkedOnly,
    /// An unlinked subject with a verified email under the tenant domain
    /// signs in as the one active member labeled with that email, and the
    /// link is recorded. Two matches or none refuse.
    VerifiedEmailDomain,
}

/// A reviewed signing key: RSA public key parts, base64url, as a JWKS lists them.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Jwk {
    pub kid: String,
    pub n: String,
    pub e: String,
}

/// The owner-reviewed provider terms for one workspace.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Terms {
    pub issuer: String,
    /// The tenant claim (`hd` for Google Workspace) every token must carry.
    pub tenant: String,
    pub audience: String,
    pub linking: Linking,
    pub keys: Vec<Jwk>,
    /// Claim names the audit export may include; never a token or secret.
    pub audit_fields: BTreeSet<String>,
}
impl Terms {
    pub fn validate(&self) -> Result<(), String> {
        let short = |s: &str, n: usize| s.is_empty() || s.len() > n;
        if !self.issuer.starts_with("https://")
            || short(&self.issuer, 256)
            || short(&self.tenant, 256)
            || short(&self.audience, 256)
            || self.keys.is_empty()
            || self.keys.len() > 16
            || self.audit_fields.len() > 16
            || self.keys.iter().any(|k| {
                short(&k.kid, 128) || short(&k.n, 4096) || short(&k.e, 16) || !b64url_ok(&k.n)
            })
        {
            return Err("Invalid SSO provider terms.".into());
        }
        let mut kids = BTreeSet::new();
        if !self.keys.iter().all(|k| kids.insert(&k.kid)) {
            return Err("SSO provider keys repeat a key id.".into());
        }
        if self
            .audit_fields
            .iter()
            .any(|f| !["sub", "email", "hd", "iat", "exp", "nonce"].contains(&f.as_str()))
        {
            return Err("SSO audit fields name a claim this book does not export.".into());
        }
        Ok(())
    }
    pub fn digest(&self) -> String {
        let mut h = Sha256::new();
        h.update(SCHEMA);
        h.update(serde_json::to_vec(self).unwrap_or_default());
        hex(&h.finalize())
    }
    pub fn key(&self, kid: &str) -> Option<&Jwk> {
        self.keys.iter().find(|k| k.kid == kid)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Revision {
    pub workspace: String,
    pub version: u32,
    pub digest: String,
    pub supersedes: Option<String>,
    pub owner: String,
    pub reviewer: String,
    pub recorded_at: u64,
    pub terms: Terms,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Link {
    pub workspace: String,
    pub account: String,
    pub provider_digest: String,
    pub linked_by: String,
    pub linked_at: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Outcome {
    SignedIn,
    Refused,
    Linked,
    Unlinked,
    Configured,
}

/// One audit row: references and digests, never a credential or claim body.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Audit {
    pub at: u64,
    pub workspace: String,
    pub outcome: Outcome,
    pub provider_digest: Option<String>,
    pub subject_digest: Option<String>,
    pub account: Option<String>,
    pub actor: Option<String>,
    pub reason: String,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub fields: BTreeMap<String, String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Book {
    pub providers: BTreeMap<String, Vec<Revision>>,
    /// Principal `sso:<digest>` to its link.
    pub links: BTreeMap<String, Link>,
    /// Consumed token ids (`jti` or nonce digest) and their expiry.
    pub consumed: BTreeMap<String, u64>,
    pub audit: Vec<Audit>,
}
impl Book {
    pub fn is_empty(&self) -> bool {
        self.providers.is_empty()
            && self.links.is_empty()
            && self.consumed.is_empty()
            && self.audit.is_empty()
    }
    pub(super) fn validate(&self, store: &Store) -> Result<(), String> {
        if self.providers.len() > 1024
            || self.links.len() > 65_536
            || self.consumed.len() > 65_536
            || self.audit.len() > MAX_AUDIT
        {
            return Err("SSO history exceeds its bound.".into());
        }
        for (ws, history) in &self.providers {
            if !store.workspaces.contains_key(ws) || history.is_empty() || history.len() > 128 {
                return Err("Invalid SSO provider history.".into());
            }
            let mut previous: Option<&Revision> = None;
            for r in history {
                r.terms.validate()?;
                if r.workspace != *ws
                    || r.digest != r.terms.digest()
                    || r.version != previous.map_or(1, |p| p.version + 1)
                    || r.supersedes.as_deref() != previous.map(|p| p.digest.as_str())
                    || !store.accounts.contains_key(&r.owner)
                    || !store.accounts.contains_key(&r.reviewer)
                {
                    return Err("Invalid SSO provider lineage.".into());
                }
                previous = Some(r);
            }
        }
        for (principal, link) in &self.links {
            let account = store
                .accounts
                .get(&link.account)
                .ok_or("SSO link names no account.")?;
            if !principal.starts_with("sso:")
                || !account.principals.contains(principal)
                || !store.workspaces.contains_key(&link.workspace)
                || !store.accounts.contains_key(&link.linked_by)
            {
                return Err("Invalid SSO link.".into());
            }
        }
        Ok(())
    }
    pub fn current(&self, workspace: &str) -> Option<&Revision> {
        self.providers.get(workspace)?.last()
    }
    fn record(&mut self, row: Audit) {
        if self.audit.len() >= MAX_AUDIT {
            self.audit.remove(0);
        }
        self.audit.push(row);
    }
}

/// The claims this book reads. Unknown claims are ignored, never kept.
#[derive(Clone, Debug, Default, Deserialize)]
pub struct Claims {
    #[serde(default)]
    pub iss: String,
    #[serde(default)]
    pub aud: Audience,
    #[serde(default)]
    pub sub: String,
    #[serde(default)]
    pub exp: u64,
    #[serde(default)]
    pub iat: u64,
    #[serde(default)]
    pub hd: Option<String>,
    #[serde(default)]
    pub email: Option<String>,
    #[serde(default)]
    pub email_verified: bool,
    #[serde(default)]
    pub nonce: Option<String>,
    #[serde(default)]
    pub jti: Option<String>,
}
#[derive(Clone, Debug, Deserialize)]
#[serde(untagged)]
pub enum Audience {
    One(String),
    Many(Vec<String>),
}
impl Default for Audience {
    fn default() -> Self {
        Self::One(String::new())
    }
}
impl Audience {
    fn contains(&self, aud: &str) -> bool {
        match self {
            Self::One(a) => a == aud,
            Self::Many(all) => all.iter().any(|a| a == aud),
        }
    }
}

#[derive(Debug, Deserialize)]
struct Header {
    #[serde(default)]
    alg: String,
    #[serde(default)]
    kid: String,
}

/// A parsed compact JWS: the signing input, the signature, and the claims.
pub struct Token {
    pub kid: String,
    pub alg: String,
    pub signing_input: Vec<u8>,
    pub signature: Vec<u8>,
    pub claims: Claims,
}
impl Token {
    pub fn parse(compact: &str) -> Result<Self, SsoRefusal> {
        if compact.len() > 16_384 {
            return Err(SsoRefusal::Malformed);
        }
        let mut parts = compact.split('.');
        let (h, c, s) = match (parts.next(), parts.next(), parts.next(), parts.next()) {
            (Some(h), Some(c), Some(s), None) => (h, c, s),
            _ => return Err(SsoRefusal::Malformed),
        };
        let header: Header =
            serde_json::from_slice(&b64url(h)?).map_err(|_| SsoRefusal::Malformed)?;
        let claims: Claims =
            serde_json::from_slice(&b64url(c)?).map_err(|_| SsoRefusal::Malformed)?;
        Ok(Self {
            kid: header.kid,
            alg: header.alg,
            signing_input: format!("{h}.{c}").into_bytes(),
            signature: b64url(s)?,
            claims,
        })
    }
}

/// Checks one signature against one reviewed key. The gateway supplies RS256;
/// tests supply the fixture verifier.
pub trait Verifier {
    fn verify(&self, alg: &str, key: &Jwk, signing_input: &[u8], signature: &[u8]) -> bool;
}
/// Accepts `alg = "fixture"` and a signature equal to
/// `sha256(kid || "." || signing_input || "." || secret)`. For tests only.
pub struct FixtureVerifier {
    pub secret: Vec<u8>,
}
impl FixtureVerifier {
    pub fn sign(&self, kid: &str, signing_input: &[u8]) -> Vec<u8> {
        let mut h = Sha256::new();
        h.update(kid.as_bytes());
        h.update(b".");
        h.update(signing_input);
        h.update(b".");
        h.update(&self.secret);
        h.finalize().to_vec()
    }
}
impl Verifier for FixtureVerifier {
    fn verify(&self, alg: &str, key: &Jwk, signing_input: &[u8], signature: &[u8]) -> bool {
        alg == "fixture" && self.sign(&key.kid, signing_input) == signature
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SsoRefusal {
    Malformed,
    NoProvider(String),
    /// Issuer, audience, tenant, signature, lifetime, or replay failed.
    /// One answer for all of them: the token learns nothing.
    Denied,
    /// The subject is verified and maps to no single admitted member.
    Unlinked,
    NotMember {
        workspace: String,
        account: String,
    },
    Terms(String),
}
impl std::fmt::Display for SsoRefusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Malformed => f.write_str("the SSO token is not a compact JWS"),
            Self::NoProvider(ws) => write!(f, "workspace `{ws}` admits no SSO provider"),
            Self::Denied => f.write_str("SSO sign-in denied"),
            Self::Unlinked => {
                f.write_str("the verified subject is not linked to one member of this workspace")
            }
            Self::NotMember { workspace, account } => {
                write!(f, "`{account}` holds no active membership in `{workspace}`")
            }
            Self::Terms(m) => f.write_str(m),
        }
    }
}

/// A sign-in the book admitted. The caller mints the session.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Admitted {
    pub workspace: String,
    pub account: String,
    pub role: Role,
    pub provider_digest: String,
    pub subject_digest: String,
}

pub fn principal(issuer: &str, sub: &str) -> String {
    let mut h = Sha256::new();
    h.update(issuer.as_bytes());
    h.update(b"\n");
    h.update(sub.as_bytes());
    format!("sso:{}", hex(&h.finalize()))
}

impl super::Accounts {
    /// The workspace's current provider revision, when one is recorded.
    pub fn sso_current(&self, workspace: &str) -> Result<Option<Revision>, Refusal> {
        let store = super::load(&self.dir).map_err(Refusal::Store)?;
        Ok(store.sso.current(workspace).cloned())
    }

    /// Record reviewed provider terms for a workspace. Owner only; the
    /// reviewer is a named account. A new revision supersedes the last.
    pub fn sso_configure(
        &self,
        actor: &str,
        workspace: &str,
        reviewer: &str,
        terms: Terms,
    ) -> Result<Revision, Refusal> {
        terms
            .validate()
            .map_err(|m| Refusal::Sso(SsoRefusal::Terms(m)))?;
        self.mutate(|store, now| {
            let ws = store
                .workspaces
                .get(workspace)
                .ok_or_else(|| Refusal::UnknownWorkspace(workspace.into()))?;
            if ws.kind == super::WorkspaceKind::Personal {
                return Err(Refusal::PersonalWorkspace(workspace.into()));
            }
            if active_member(ws, actor)?.role != Role::Owner {
                return Err(Refusal::Forbidden {
                    workspace: workspace.into(),
                    account: actor.into(),
                    action: "configure SSO",
                });
            }
            if !store.accounts.contains_key(reviewer) {
                return Err(Refusal::UnknownAccount(reviewer.into()));
            }
            let previous = store.sso.current(workspace).cloned();
            let revision = Revision {
                workspace: workspace.into(),
                version: previous.as_ref().map_or(1, |p| p.version + 1),
                digest: terms.digest(),
                supersedes: previous.as_ref().map(|p| p.digest.clone()),
                owner: actor.into(),
                reviewer: reviewer.into(),
                recorded_at: now,
                terms,
            };
            store
                .sso
                .providers
                .entry(workspace.into())
                .or_default()
                .push(revision.clone());
            store.sso.record(Audit {
                at: now,
                workspace: workspace.into(),
                outcome: Outcome::Configured,
                provider_digest: Some(revision.digest.clone()),
                subject_digest: None,
                account: None,
                actor: Some(actor.into()),
                reason: format!("version {}", revision.version),
                fields: BTreeMap::new(),
            });
            Ok(revision)
        })
    }

    /// Bind a provider subject to an account that is an active member.
    /// Admin or owner only. A subject binds to one account anywhere.
    pub fn sso_link(
        &self,
        actor: &str,
        workspace: &str,
        account: &str,
        sub: &str,
    ) -> Result<Link, Refusal> {
        self.mutate(|store, now| {
            let ws = store
                .workspaces
                .get(workspace)
                .ok_or_else(|| Refusal::UnknownWorkspace(workspace.into()))?;
            if active_member(ws, actor)?.role == Role::Member {
                return Err(Refusal::Forbidden {
                    workspace: workspace.into(),
                    account: actor.into(),
                    action: "link SSO subjects",
                });
            }
            active_member(ws, account)?;
            let revision = store
                .sso
                .current(workspace)
                .ok_or_else(|| Refusal::Sso(SsoRefusal::NoProvider(workspace.into())))?;
            let principal = principal(&revision.terms.issuer, sub);
            let provider_digest = revision.digest.clone();
            if let Some(other) = store
                .accounts
                .values()
                .find(|a| a.principals.contains(&principal))
            {
                return Err(Refusal::PrincipalTaken {
                    principal,
                    account: other.id.clone(),
                });
            }
            store
                .accounts
                .get_mut(account)
                .ok_or_else(|| Refusal::UnknownAccount(account.into()))?
                .principals
                .push(principal.clone());
            let link = Link {
                workspace: workspace.into(),
                account: account.into(),
                provider_digest: provider_digest.clone(),
                linked_by: actor.into(),
                linked_at: now,
            };
            store.sso.links.insert(principal.clone(), link.clone());
            store.sso.record(Audit {
                at: now,
                workspace: workspace.into(),
                outcome: Outcome::Linked,
                provider_digest: Some(provider_digest),
                subject_digest: Some(principal),
                account: Some(account.into()),
                actor: Some(actor.into()),
                reason: "admin link".into(),
                fields: BTreeMap::new(),
            });
            Ok(link)
        })
    }

    /// Remove a subject's binding. Admin or owner, or the linked account itself.
    pub fn sso_unlink(&self, actor: &str, workspace: &str, sub: &str) -> Result<Link, Refusal> {
        self.mutate(|store, now| {
            let ws = store
                .workspaces
                .get(workspace)
                .ok_or_else(|| Refusal::UnknownWorkspace(workspace.into()))?;
            let revision = store
                .sso
                .current(workspace)
                .ok_or_else(|| Refusal::Sso(SsoRefusal::NoProvider(workspace.into())))?;
            let principal = principal(&revision.terms.issuer, sub);
            let link = store
                .sso
                .links
                .get(&principal)
                .cloned()
                .ok_or_else(|| Refusal::UnknownPrincipal(principal.clone()))?;
            let actor_role = active_member(ws, actor)?.role;
            if actor_role == Role::Member && actor != link.account {
                return Err(Refusal::Forbidden {
                    workspace: workspace.into(),
                    account: actor.into(),
                    action: "unlink SSO subjects",
                });
            }
            if let Some(a) = store.accounts.get_mut(&link.account) {
                a.principals.retain(|p| p != &principal);
            }
            store.sso.links.remove(&principal);
            store.sso.record(Audit {
                at: now,
                workspace: workspace.into(),
                outcome: Outcome::Unlinked,
                provider_digest: Some(link.provider_digest.clone()),
                subject_digest: Some(principal),
                account: Some(link.account.clone()),
                actor: Some(actor.into()),
                reason: "unlink".into(),
                fields: BTreeMap::new(),
            });
            Ok(link)
        })
    }

    /// Verify an ID token against the workspace's current provider and
    /// resolve the member it signs in. Every refusal after parsing is
    /// recorded in the audit, with digests only. A provider outage is a
    /// caller problem before this call; nothing here falls back.
    pub fn sso_sign_in(
        &self,
        workspace: &str,
        compact: &str,
        verifier: &dyn Verifier,
    ) -> Result<Admitted, Refusal> {
        let token = Token::parse(compact).map_err(Refusal::Sso)?;
        // Refusals return `Ok(Err(_))` so the audit row they record is saved.
        self.mutate(|store, now| {
            let revision = store
                .sso
                .current(workspace)
                .cloned()
                .ok_or_else(|| Refusal::Sso(SsoRefusal::NoProvider(workspace.into())))?;
            let terms = &revision.terms;
            let c = &token.claims;
            let principal = principal(&terms.issuer, &c.sub);
            let refuse = |store: &mut Store, reason: &str, refusal: SsoRefusal| {
                store.sso.record(Audit {
                    at: now,
                    workspace: workspace.into(),
                    outcome: Outcome::Refused,
                    provider_digest: Some(revision.digest.clone()),
                    subject_digest: Some(principal.clone()),
                    account: None,
                    actor: None,
                    reason: reason.into(),
                    fields: BTreeMap::new(),
                });
                Ok(Err(Refusal::Sso(refusal)))
            };
            let key = match terms.key(&token.kid) {
                Some(k) => k,
                None => return refuse(store, "unknown key id", SsoRefusal::Denied),
            };
            if c.iss != terms.issuer || !c.aud.contains(&terms.audience) {
                return refuse(store, "issuer or audience", SsoRefusal::Denied);
            }
            if c.hd.as_deref() != Some(terms.tenant.as_str()) {
                return refuse(store, "tenant", SsoRefusal::Denied);
            }
            if c.sub.is_empty() || c.sub.len() > 256 {
                return refuse(store, "subject", SsoRefusal::Denied);
            }
            if c.exp <= now
                || c.iat > now + MAX_SKEW_SECS
                || c.exp > c.iat + MAX_TOKEN_LIFETIME_SECS
            {
                return refuse(store, "lifetime", SsoRefusal::Denied);
            }
            if !verifier.verify(&token.alg, key, &token.signing_input, &token.signature) {
                return refuse(store, "signature", SsoRefusal::Denied);
            }
            let replay_key = {
                let mut h = Sha256::new();
                h.update(&token.signature);
                hex(&h.finalize())
            };
            store.sso.consumed.retain(|_, exp| *exp > now);
            if store.sso.consumed.contains_key(&replay_key) {
                return refuse(store, "replay", SsoRefusal::Denied);
            }
            let ws = store
                .workspaces
                .get(workspace)
                .ok_or_else(|| Refusal::UnknownWorkspace(workspace.into()))?;
            let linked = store
                .accounts
                .values()
                .find(|a| a.principals.contains(&principal))
                .map(|a| a.id.clone());
            let account = match linked {
                Some(a) => a,
                None if terms.linking == Linking::VerifiedEmailDomain => {
                    let email = c.email.as_deref().unwrap_or_default().to_ascii_lowercase();
                    let domain_ok = email
                        .rsplit_once('@')
                        .is_some_and(|(_, d)| d == terms.tenant.to_ascii_lowercase());
                    if !c.email_verified || !domain_ok {
                        return refuse(store, "email not verified in tenant", SsoRefusal::Unlinked);
                    }
                    let mut matches = ws.members.values().filter(|m| {
                        m.status == MemberStatus::Active
                            && store
                                .accounts
                                .get(&m.account)
                                .is_some_and(|a| a.label.to_ascii_lowercase() == email)
                    });
                    match (matches.next(), matches.next()) {
                        (Some(m), None) => m.account.clone(),
                        _ => {
                            return refuse(
                                store,
                                "ambiguous or no email match",
                                SsoRefusal::Unlinked,
                            );
                        }
                    }
                }
                None => return refuse(store, "unlinked", SsoRefusal::Unlinked),
            };
            let role = match active_member(ws, &account) {
                Ok(m) => m.role,
                Err(_) => {
                    return refuse(
                        store,
                        "membership removed",
                        SsoRefusal::NotMember {
                            workspace: workspace.into(),
                            account: account.clone(),
                        },
                    );
                }
            };
            if !store.sso.links.contains_key(&principal) {
                store
                    .accounts
                    .get_mut(&account)
                    .ok_or_else(|| Refusal::UnknownAccount(account.clone()))?
                    .principals
                    .push(principal.clone());
                store.sso.links.insert(
                    principal.clone(),
                    Link {
                        workspace: workspace.into(),
                        account: account.clone(),
                        provider_digest: revision.digest.clone(),
                        linked_by: account.clone(),
                        linked_at: now,
                    },
                );
            }
            store.sso.consumed.insert(replay_key, c.exp);
            let mut fields = BTreeMap::new();
            for f in &terms.audit_fields {
                let v = match f.as_str() {
                    "sub" => Some(c.sub.clone()),
                    "email" => c.email.clone(),
                    "hd" => c.hd.clone(),
                    "iat" => Some(c.iat.to_string()),
                    "exp" => Some(c.exp.to_string()),
                    "nonce" => c.nonce.clone(),
                    _ => None,
                };
                if let Some(v) = v {
                    fields.insert(f.clone(), v);
                }
            }
            store.sso.record(Audit {
                at: now,
                workspace: workspace.into(),
                outcome: Outcome::SignedIn,
                provider_digest: Some(revision.digest.clone()),
                subject_digest: Some(principal.clone()),
                account: Some(account.clone()),
                actor: None,
                reason: "verified".into(),
                fields,
            });
            Ok(Ok(Admitted {
                workspace: workspace.into(),
                account,
                role,
                provider_digest: revision.digest.clone(),
                subject_digest: principal,
            }))
        })?
    }

    /// The workspace's SSO audit rows since `since`, for an admin or owner.
    /// Rows of other workspaces are never returned.
    pub fn sso_audit(
        &self,
        actor: &str,
        workspace: &str,
        since: u64,
    ) -> Result<Vec<Audit>, Refusal> {
        let _lock = Lock::acquire(&self.dir).map_err(Refusal::Store)?;
        let store = super::load(&self.dir).map_err(Refusal::Store)?;
        let ws = store
            .workspaces
            .get(workspace)
            .ok_or_else(|| Refusal::UnknownWorkspace(workspace.into()))?;
        if active_member(ws, actor)?.role == Role::Member {
            return Err(Refusal::Forbidden {
                workspace: workspace.into(),
                account: actor.into(),
                action: "export SSO audit",
            });
        }
        Ok(store
            .sso
            .audit
            .iter()
            .filter(|a| a.workspace == workspace && a.at >= since)
            .cloned()
            .collect())
    }
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}
fn b64url_ok(s: &str) -> bool {
    !s.is_empty()
        && s.bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}
/// Base64url without padding, as RFC 7515 uses it.
pub fn b64url(s: &str) -> Result<Vec<u8>, SsoRefusal> {
    if !b64url_ok(s) {
        return Err(SsoRefusal::Malformed);
    }
    let val = |b: u8| -> u32 {
        match b {
            b'A'..=b'Z' => (b - b'A') as u32,
            b'a'..=b'z' => (b - b'a' + 26) as u32,
            b'0'..=b'9' => (b - b'0' + 52) as u32,
            b'-' => 62,
            _ => 63,
        }
    };
    let mut out = Vec::with_capacity(s.len() * 3 / 4);
    for chunk in s.as_bytes().chunks(4) {
        let mut acc = 0u32;
        for (i, b) in chunk.iter().enumerate() {
            acc |= val(*b) << (18 - 6 * i);
        }
        let bytes = acc.to_be_bytes();
        match chunk.len() {
            4 => out.extend_from_slice(&bytes[1..4]),
            3 => out.extend_from_slice(&bytes[1..3]),
            2 => out.push(bytes[1]),
            _ => return Err(SsoRefusal::Malformed),
        }
    }
    Ok(out)
}
pub fn b64url_encode(bytes: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
    let mut out = String::new();
    for chunk in bytes.chunks(3) {
        let mut acc = 0u32;
        for (i, b) in chunk.iter().enumerate() {
            acc |= (*b as u32) << (16 - 8 * i);
        }
        for i in 0..=chunk.len() {
            out.push(T[((acc >> (18 - 6 * i)) & 63) as usize] as char);
        }
    }
    out
}
