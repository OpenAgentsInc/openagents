//! Authorization precedes secret access and is checked again before delivery.
//! Values belong only in the inspected host-side proxy, never in guest metadata.
pub mod certificate_authority;
pub mod gateway;
#[cfg(unix)]
pub mod host_transport;
pub mod policy;
pub mod secret_manager;
use async_trait::async_trait;
use base64::{Engine as _, engine::general_purpose::STANDARD};
use hmac::{Hmac, Mac};
use policy::{Destination, Profile};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, fmt, sync::Arc};
use subtle::ConstantTimeEq;
use uuid::Uuid;
use zeroize::Zeroizing;

pub const MAX_HANDLE_SECONDS: u64 = 3600;
pub const MAX_SECRET_BYTES: usize = 16384;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Failure {
    Denied,
    Unavailable,
    Invalid,
}
impl fmt::Display for Failure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Denied => "Credential access was denied.",
            Self::Unavailable => "Credential access is unavailable.",
            Self::Invalid => "The credential settings are invalid.",
        })
    }
}
impl std::error::Error for Failure {}

pub struct Secret(Zeroizing<Vec<u8>>);
impl Secret {
    pub fn new(bytes: Vec<u8>) -> Result<Self, Failure> {
        let bytes = Zeroizing::new(bytes);
        if bytes.is_empty() || bytes.len() > MAX_SECRET_BYTES {
            return Err(Failure::Invalid);
        }
        Ok(Self(bytes))
    }
    pub fn bytes(&self) -> &[u8] {
        &self.0
    }
}
impl fmt::Debug for Secret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Secret(<redacted>)")
    }
}
/// Authority for the host-side proxy. Public reference metadata never includes it.
pub struct ProxyToken(Zeroizing<String>);
impl ProxyToken {
    pub fn expose(&self) -> &str {
        &self.0
    }
}
impl fmt::Debug for ProxyToken {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("ProxyToken(<redacted>)")
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Workspace {
    pub id: String,
    pub members_epoch: u64,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Owner {
    Account { id: String },
    Workspace { id: String },
}
impl Owner {
    fn admits(&self, owner: &str, workspace: Option<&Workspace>) -> bool {
        match self {
            Self::Account { id } => id == owner,
            Self::Workspace { id } => workspace.is_some_and(|w| &w.id == id),
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Encoding {
    Plain,
    Basic { username: String },
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Rule {
    pub destination: Destination,
    pub header: String,
    pub prefix: String,
    pub encoding: Encoding,
}
impl Rule {
    pub fn validate(&self) -> Result<(), Failure> {
        self.destination.validate().map_err(|_| Failure::Invalid)?;
        if !matches!(
            self.header.as_str(),
            "authorization" | "x-api-key" | "x-goog-api-key"
        ) || self.prefix.len() > 32
            || !self.prefix.bytes().all(|b| (b' '..=b'~').contains(&b))
            || matches!(&self.encoding, Encoding::Basic{username} if username.is_empty() || username.len()>128 || username.contains(':') || !username.bytes().all(|b| (b'!'..=b'~').contains(&b)))
        {
            return Err(Failure::Invalid);
        }
        Ok(())
    }
    fn header_value(&self, secret: &Secret) -> Result<Secret, Failure> {
        let mut value = Zeroizing::new(self.prefix.as_bytes().to_vec());
        match &self.encoding {
            Encoding::Plain => value.extend_from_slice(secret.bytes()),
            Encoding::Basic { username } => {
                let mut plain = Zeroizing::new(username.as_bytes().to_vec());
                plain.push(b':');
                plain.extend_from_slice(secret.bytes());
                let encoded = Zeroizing::new(STANDARD.encode(plain.as_slice()));
                value.extend_from_slice(encoded.as_bytes());
            }
        }
        if !value.iter().all(|b| (b' '..=b'~').contains(b)) {
            return Err(Failure::Invalid);
        }
        Secret::new(std::mem::take(&mut *value))
    }
}

/// Trusted connection metadata. The public API selects `id` and a rule name;
/// it never accepts a cloud resource name from a caller.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Source {
    pub id: Uuid,
    pub revision: u64,
    pub owner: Owner,
    pub secret_version: secret_manager::Version,
    pub rules: BTreeMap<String, Rule>,
}
impl Source {
    pub fn validate(&self) -> Result<(), Failure> {
        if self.id.is_nil()
            || self.revision == 0
            || self.rules.is_empty()
            || self.rules.len() > 32
            || self.rules.keys().any(|k| !opaque(k, 64))
        {
            return Err(Failure::Invalid);
        }
        let (Owner::Account { id } | Owner::Workspace { id }) = &self.owner;
        if !opaque(id, 128) {
            return Err(Failure::Invalid);
        }
        for rule in self.rules.values() {
            rule.validate()?;
        }
        Ok(())
    }
    pub fn digest(&self) -> Result<String, Failure> {
        self.validate()?;
        let mut hash = Sha256::new();
        hash.update(b"openagents.computers.credential-source.v1\0");
        hash.update(serde_json::to_vec(self).map_err(|_| Failure::Invalid)?);
        Ok(format!("{:x}", hash.finalize()))
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum State {
    Prepared,
    Active,
    Revoked,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Binding {
    pub id: Uuid,
    pub revision: u64,
    pub state: State,
    pub source: Uuid,
    pub source_revision: u64,
    pub source_digest: String,
    pub rule: String,
    pub owner: String,
    pub workspace: Option<Workspace>,
    pub grant_revision: u64,
    pub computer: String,
    pub native_resource: String,
    pub node: String,
    pub execution_generation: u64,
    pub native_policy_ref: String,
    pub command_key: String,
    pub command_digest: Option<String>,
    pub profile: Profile,
    pub created_unix: u64,
    pub expires_unix: u64,
}
impl Binding {
    pub fn validate(&self) -> Result<(), Failure> {
        self.profile.validate().map_err(|_| Failure::Invalid)?;
        if self.id.is_nil()
            || self.source.is_nil()
            || self.revision == 0
            || self.source_revision == 0
            || !digest(&self.source_digest)
            || !opaque(&self.rule, 64)
            || !opaque(&self.owner, 128)
            || !opaque(&self.computer, 128)
            || !opaque(&self.native_resource, 128)
            || !opaque(&self.node, 128)
            || self.execution_generation == 0
            || !opaque(&self.native_policy_ref, 256)
            || !opaque(&self.command_key, 128)
            || self.workspace.as_ref().is_some_and(|w| !opaque(&w.id, 128))
            || self.command_digest.as_ref().is_some_and(|d| !digest(d))
            || self.expires_unix <= self.created_unix
            || self.expires_unix - self.created_unix > MAX_HANDLE_SECONDS
        {
            return Err(Failure::Invalid);
        }
        Ok(())
    }
}
pub fn opaque(value: &str, max: usize) -> bool {
    !value.is_empty()
        && value.len() <= max
        && !value.starts_with('.')
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"_-.:".contains(&b))
}
pub fn digest(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

/// Host-side request context. There is no caller-supplied account or cloud path.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Use {
    pub reference: Uuid,
    pub native_resource: String,
    pub node: String,
    pub execution_generation: u64,
    pub native_policy_ref: String,
    pub host: String,
    pub port: u16,
    pub path: String,
    pub method: String,
    pub tls_inspected: bool,
}
/// Read from the current owning service and grant authority, independently of
/// the proxy's request. A database or authorization outage returns an error.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Current {
    pub owner: String,
    pub workspace: Option<Workspace>,
    pub computer: String,
    pub native_resource: String,
    pub node: String,
    pub execution_generation: u64,
    pub native_policy_ref: String,
    pub command_key: String,
    pub command_digest: String,
    pub policy_digest: String,
    pub source_revision: u64,
    pub grant_revision: u64,
    pub running: bool,
    pub grant_active: bool,
}
#[async_trait]
pub trait Store: Send + Sync {
    async fn get(&self, id: Uuid) -> Result<Option<Binding>, Failure>;
}
#[async_trait]
pub trait Authorizer: Send + Sync {
    async fn current(&self, binding: &Binding) -> Result<Current, Failure>;
}
#[async_trait]
pub trait SecretProvider: Send + Sync {
    async fn access(&self, version: &secret_manager::Version) -> Result<Secret, Failure>;
}
pub trait Clock: Send + Sync {
    fn now_unix(&self) -> u64;
}
pub struct SystemClock;
impl Clock for SystemClock {
    fn now_unix(&self) -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(u64::MAX, |d| d.as_secs())
    }
}

pub struct Broker {
    store: Arc<dyn Store>,
    authorizer: Arc<dyn Authorizer>,
    provider: Arc<dyn SecretProvider>,
    clock: Arc<dyn Clock>,
    sources: BTreeMap<Uuid, Source>,
    signing_key: Secret,
}
impl Broker {
    pub fn new(
        store: Arc<dyn Store>,
        authorizer: Arc<dyn Authorizer>,
        provider: Arc<dyn SecretProvider>,
        clock: Arc<dyn Clock>,
        sources: Vec<Source>,
        signing_key: Secret,
    ) -> Result<Self, Failure> {
        if signing_key.bytes().len() < 32 {
            return Err(Failure::Invalid);
        }
        let mut catalog = BTreeMap::new();
        for source in sources {
            source.validate()?;
            if catalog.insert(source.id, source).is_some() {
                return Err(Failure::Invalid);
            }
        }
        Ok(Self {
            store,
            authorizer,
            provider,
            clock,
            sources: catalog,
            signing_key,
        })
    }
    fn token_for(&self, b: &Binding) -> Result<ProxyToken, Failure> {
        let mut mac = Hmac::<Sha256>::new_from_slice(self.signing_key.bytes())
            .map_err(|_| Failure::Invalid)?;
        mac.update(b"openagents.computers.credential-proxy.v1\0");
        mac.update(
            &serde_json::to_vec(&serde_json::json!([
                b.id,
                b.source,
                b.source_revision,
                b.source_digest,
                b.rule,
                b.owner,
                b.workspace,
                b.grant_revision,
                b.computer,
                b.native_resource,
                b.node,
                b.execution_generation,
                b.command_key,
                b.profile.digest().map_err(|_| Failure::Invalid)?,
                b.created_unix,
                b.expires_unix
            ]))
            .map_err(|_| Failure::Invalid)?,
        );
        Ok(ProxyToken(Zeroizing::new(format!(
            "cpx_{}.{:x}",
            b.id,
            mac.finalize().into_bytes()
        ))))
    }
    /// Called only by the trusted registration path that installs host material.
    /// A prepared reference still cannot resolve before command admission.
    pub async fn proxy_token(&self, id: Uuid) -> Result<ProxyToken, Failure> {
        let b = self.store.get(id).await?.ok_or(Failure::Denied)?;
        b.validate()?;
        if b.id != id || b.state == State::Revoked || self.clock.now_unix() >= b.expires_unix {
            return Err(Failure::Denied);
        }
        self.token_for(&b)
    }
    async fn authorized(&self, token: &str, request: &Use) -> Result<(Binding, Current), Failure> {
        let b = self
            .store
            .get(request.reference)
            .await?
            .ok_or(Failure::Denied)?;
        b.validate().map_err(|_| Failure::Denied)?;
        let expected = self.token_for(&b)?;
        if token
            .as_bytes()
            .ct_eq(expected.expose().as_bytes())
            .unwrap_u8()
            != 1
        {
            return Err(Failure::Denied);
        }
        let source = self.sources.get(&b.source).ok_or(Failure::Denied)?;
        let rule = source.rules.get(&b.rule).ok_or(Failure::Denied)?;
        let profile_digest = b.profile.digest().map_err(|_| Failure::Denied)?;
        let now = self.clock.now_unix();
        if b.id != request.reference
            || b.state != State::Active
            || now < b.created_unix
            || now >= b.expires_unix
            || b.source_revision != source.revision
            || b.source_digest != source.digest()?
            || !source.owner.admits(&b.owner, b.workspace.as_ref())
            || request.native_resource != b.native_resource
            || request.node != b.node
            || request.execution_generation != b.execution_generation
            || request.native_policy_ref != b.native_policy_ref
            || !request.tls_inspected
            || !b.profile.allow_hosts.contains(&rule.destination.host)
            || b.profile.tls_exempt_hosts.contains(&rule.destination.host)
            || !rule.destination.matches(
                &request.host,
                request.port,
                &request.path,
                &request.method,
            )
        {
            return Err(Failure::Denied);
        }
        let current = self.authorizer.current(&b).await?;
        if !current.running
            || !current.grant_active
            || current.owner != b.owner
            || current.workspace != b.workspace
            || current.computer != b.computer
            || current.native_resource != b.native_resource
            || current.node != b.node
            || current.execution_generation != b.execution_generation
            || current.native_policy_ref != b.native_policy_ref
            || current.command_key != b.command_key
            || b.command_digest.as_ref() != Some(&current.command_digest)
            || !digest(&current.command_digest)
            || current.policy_digest != profile_digest
            || current.source_revision != b.source_revision
            || current.grant_revision != b.grant_revision
        {
            return Err(Failure::Denied);
        }
        Ok((b, current))
    }
    /// A stream watcher can recheck authority without reading another secret.
    pub async fn check(&self, token: &str, request: &Use) -> Result<(), Failure> {
        self.authorized(token, request).await.map(|_| ())
    }
    pub async fn resolve(&self, token: &str, request: &Use) -> Result<(String, Secret), Failure> {
        let (binding, current) = self.authorized(token, request).await?;
        let source = self.sources.get(&binding.source).ok_or(Failure::Denied)?;
        let secret = self.provider.access(&source.secret_version).await?;
        let (after_binding, after) = self.authorized(token, request).await?;
        if binding != after_binding || current != after {
            return Err(Failure::Denied);
        }
        let rule = source.rules.get(&binding.rule).ok_or(Failure::Denied)?;
        Ok((rule.header.clone(), rule.header_value(&secret)?))
    }
}

#[cfg(test)]
mod tests;
