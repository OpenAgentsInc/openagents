//! NIP-ATT records: the attested workload release (`3202`), the release
//! head (`30202`), and the attested endpoint (`30203`), with the binding
//! that ties an endpoint key to a release, release admission under the
//! head's notice delay, and the sealed-job fields a decision request
//! carries (`openagents.attested.v1`).
//!
//! Everything here is pure and has no vendor cryptography: verifying the
//! hardware evidence an endpoint carries (a Google Confidential Space
//! token, an Intel or AMD report) belongs to a verifier that knows the
//! vendor's roots (`crates/oa-att`). This module checks what the Nostr
//! signatures cover and recomputes the binding the evidence must carry.
//! See `nips/openagents/NIP-ATT.md`.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use crate::contracts::jcs;
use crate::domain::{Event, RelaySigner, Tag};

/// The release kind (regular).
pub const RELEASE_KIND: u16 = crate::kinds::ATT_RELEASE;
/// The release head kind (addressable).
pub const HEAD_KIND: u16 = crate::kinds::ATT_HEAD;
/// The attested endpoint kind (addressable).
pub const ENDPOINT_KIND: u16 = crate::kinds::ATT_ENDPOINT;

/// The release body version.
pub const RELEASE_V: &str = "openagents.att-release.v1";
/// The head body version.
pub const HEAD_V: &str = "openagents.att-head.v1";
/// The endpoint body version.
pub const ENDPOINT_V: &str = "openagents.att-endpoint.v1";

/// The release's `t` marker.
pub const RELEASE_MARKER: &str = "oa:att-release:v1";
/// The head's `t` marker.
pub const HEAD_MARKER: &str = "oa:att-head:v1";
/// The endpoint's `t` marker.
pub const ENDPOINT_MARKER: &str = "oa:att-endpoint:v1";

/// The feature a sealed CJ or DEC request lists in `requires`.
pub const ATTESTED_FEATURE: &str = "openagents.attested.v1";
/// The audience a Confidential Space workload asks its token for.
pub const TOKEN_AUDIENCE: &str = "openagents.att.v1";
/// The domain separator the binding hashes first.
pub const BINDING_DOMAIN: &[u8] = b"openagents.att.v1\0";
/// An endpoint is valid for at most this long after `issued_at`.
pub const MAX_ENDPOINT_VALIDITY_SECS: u64 = 3_600;
/// The shortest notice an emergency release may take.
pub const MIN_EMERGENCY_NOTICE_SECS: u64 = 86_400;
/// An endpoint issued further in the future than this is refused.
pub const MAX_FUTURE_SKEW_SECS: u64 = 60;

/// Who can read what an endpoint is sent, computed by the client from
/// evidence and never taken from a claim.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Level {
    /// No evidence.
    Open,
    /// App Attest or a Secure Enclave-bound key chain; not private.
    Hardened,
    /// A verified TEE report on hardware the provider holds.
    Tee,
    /// As `tee`, in a named cloud provider's data center.
    TeeCloud,
}

impl Level {
    /// The wire word: `open`, `hardened`, `tee`, or `tee-cloud`.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Open => "open",
            Self::Hardened => "hardened",
            Self::Tee => "tee",
            Self::TeeCloud => "tee-cloud",
        }
    }

    /// The level a wire word names.
    #[must_use]
    pub fn parse(word: &str) -> Option<Self> {
        match word {
            "open" => Some(Self::Open),
            "hardened" => Some(Self::Hardened),
            "tee" => Some(Self::Tee),
            "tee-cloud" => Some(Self::TeeCloud),
            _ => None,
        }
    }
}

/// The image a release admits.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Image {
    /// Where the image is pulled from, without a tag or digest.
    pub reference: String,
    /// `sha256:<64 hex>`, the manifest digest.
    pub digest: String,
}

/// One platform a release is admitted on.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Platform {
    /// `gcp-confidential-space`, `tdx`, `sev-snp`, or `apple-app-attest`.
    pub kind: String,
    /// The hardware model the evidence must name, such as `GCP_INTEL_TDX`.
    pub hwmodel: String,
    /// The launcher support level the evidence must carry, `STABLE`.
    pub support: String,
}

/// A measured register for raw TDX or SEV-SNP evidence.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Measurement {
    pub register: String,
    pub value: String,
}

/// GPU evidence a release requires.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Gpu {
    pub vendor: String,
    pub mode: String,
    pub models: Vec<String>,
}

/// Weights a release serves, pinned by digest.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Model {
    /// The served model name.
    pub id: String,
    /// `sha256:<64 hex>` of the weights file (or its manifest).
    pub digest: String,
}

/// One program inside the image, by the SHA-256 of its bytes. The image
/// digest already covers these; listing them lets a reader match a
/// rebuilt binary without unpacking the image.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Component {
    /// What it is, such as `psionic-openai-server`.
    pub name: String,
    /// `sha256:<64 hex>`.
    pub digest: String,
}

/// Where the release was built from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Source {
    pub repo: String,
    /// The 40-hex commit.
    pub commit: String,
    /// The path of the build recipe in the repository.
    pub recipe: String,
    /// `sha256:<64 hex>` of the recipe file.
    pub recipe_digest: String,
}

/// An independent builder's result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Rebuild {
    pub builder: String,
    pub digest: String,
}

/// A transparency-log entry for the release.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Transparency {
    pub log: String,
    pub url: String,
    pub index: u64,
}

/// The content of a `3202` release.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Release {
    pub v: String,
    pub requires: Vec<String>,
    /// The workload slug, equal to the `w` tag.
    pub workload: String,
    /// The signer.
    pub publisher: String,
    pub image: Image,
    pub platforms: Vec<Platform>,
    pub measurements: Vec<Measurement>,
    pub gpu: Option<Gpu>,
    pub models: Vec<Model>,
    #[serde(default)]
    pub components: Vec<Component>,
    pub source: Source,
    pub rebuilds: Vec<Rebuild>,
    pub transparency: Vec<Transparency>,
    /// One paragraph for people.
    pub changes: String,
    pub published_at: u64,
}

/// One admitted release in a head.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Admitted {
    /// The release's event ID.
    pub release: String,
    pub effective_at: u64,
    pub retire_at: Option<u64>,
}

/// An emergency release, admitted after the shorter emergency notice.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Emergency {
    pub release: String,
    pub reason: String,
}

/// The content of a `30202` release head.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Head {
    pub v: String,
    pub requires: Vec<String>,
    /// The workload slug, equal to `d`.
    pub workload: String,
    /// Only grows; a reader refuses a lower one than it has seen.
    pub generation: u64,
    pub notice_seconds: u64,
    pub admitted: Vec<Admitted>,
    pub emergency: Option<Emergency>,
}

/// One piece of hardware evidence in an endpoint.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Evidence {
    /// `gcp-confidential-space-token`, `tdx-quote`, `sev-snp-report`,
    /// `nvidia-gpu`, or `apple-app-attest`.
    pub kind: String,
    /// `pki`, `oidc`, `nras-jwt`, or the raw format.
    pub format: String,
    /// The token or base64 report.
    pub token: String,
}

/// The content of a `30203` attested endpoint.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Endpoint {
    pub v: String,
    pub requires: Vec<String>,
    /// The release's event ID.
    pub release: String,
    /// The endpoint key, equal to the event's signer.
    pub endpoint: String,
    /// Base64 X25519 public key, or null.
    pub hpke: Option<String>,
    /// [`binding`] of the endpoint key, the HPKE key and the release.
    pub binding: String,
    pub evidence: Vec<Evidence>,
    /// Whoever runs the machine; informational.
    pub operator: String,
    pub issued_at: u64,
    pub valid_until: u64,
}

/// A verified `30203` event: its body and its address parts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EndpointRecord {
    pub body: Endpoint,
    /// The `d` tag: the instance ID.
    pub instance: String,
    /// The `a` tag: the head address, `30202:<publisher>:<workload>`.
    pub head: String,
}

impl EndpointRecord {
    /// This endpoint's address, `30203:<endpoint>:<instance>`.
    #[must_use]
    pub fn address(&self) -> String {
        format!("{ENDPOINT_KIND}:{}:{}", self.body.endpoint, self.instance)
    }
}

/// The NIP-ATT binding: `SHA-256("openagents.att.v1\0" || endpoint ||
/// hpke (or nothing) || release)`, each a raw 32 bytes, as lowercase hex.
///
/// # Errors
///
/// When the endpoint key or the release ID is not 64 hex characters.
pub fn binding(endpoint: &str, hpke: Option<&[u8; 32]>, release: &str) -> Result<String, String> {
    let endpoint = hex32(endpoint, "endpoint")?;
    let release = hex32(release, "release")?;
    let mut hasher = Sha256::new();
    hasher.update(BINDING_DOMAIN);
    hasher.update(endpoint);
    if let Some(hpke) = hpke {
        hasher.update(hpke);
    }
    hasher.update(release);
    Ok(to_hex(&hasher.finalize()))
}

impl Release {
    /// Check the body on its own.
    ///
    /// # Errors
    ///
    /// Names the first rule it breaks.
    pub fn validate(&self) -> Result<(), String> {
        if self.v != RELEASE_V {
            return Err("unknown release version".into());
        }
        if !self.requires.is_empty() {
            return Err("the release requires an unknown feature".into());
        }
        slug(&self.workload, "workload")?;
        hex64(&self.publisher, "publisher")?;
        if self.image.reference.is_empty() || self.image.reference.contains('@') {
            return Err("image reference is empty or carries a digest".into());
        }
        sha256_ref(&self.image.digest, "image digest")?;
        if self.platforms.is_empty() {
            return Err("a release names at least one platform".into());
        }
        for platform in &self.platforms {
            if !matches!(
                platform.kind.as_str(),
                "gcp-confidential-space" | "tdx" | "sev-snp" | "apple-app-attest"
            ) {
                return Err(format!("unknown platform kind {}", platform.kind));
            }
        }
        for model in &self.models {
            sha256_ref(&model.digest, "model digest")?;
        }
        for component in &self.components {
            sha256_ref(&component.digest, "component digest")?;
        }
        if self.source.commit.len() != 40 || !is_lower_hex(&self.source.commit) {
            return Err("source commit is not 40 lowercase hex characters".into());
        }
        sha256_ref(&self.source.recipe_digest, "recipe digest")?;
        for rebuild in &self.rebuilds {
            hex64(&rebuild.builder, "rebuild builder")?;
            sha256_ref(&rebuild.digest, "rebuild digest")?;
        }
        if self.changes.len() > 4_096 {
            return Err("changes is longer than 4096 bytes".into());
        }
        Ok(())
    }

    /// The platform entry of `kind`, if the release is admitted there.
    #[must_use]
    pub fn platform(&self, kind: &str) -> Option<&Platform> {
        self.platforms.iter().find(|p| p.kind == kind)
    }
}

impl Head {
    /// Check the body on its own.
    ///
    /// # Errors
    ///
    /// Names the first rule it breaks.
    pub fn validate(&self) -> Result<(), String> {
        if self.v != HEAD_V {
            return Err("unknown head version".into());
        }
        if !self.requires.is_empty() {
            return Err("the head requires an unknown feature".into());
        }
        slug(&self.workload, "workload")?;
        let mut seen = BTreeSet::new();
        for entry in &self.admitted {
            hex64(&entry.release, "admitted release")?;
            if !seen.insert(&entry.release) {
                return Err("a release is listed twice".into());
            }
            if entry
                .retire_at
                .is_some_and(|retire| retire <= entry.effective_at)
            {
                return Err("retire_at is not after effective_at".into());
            }
        }
        if let Some(emergency) = &self.emergency {
            hex64(&emergency.release, "emergency release")?;
            if emergency.reason.trim().is_empty() {
                return Err("an emergency names a reason".into());
            }
        }
        Ok(())
    }

    /// Whether `release` (event ID `release_id`) is admitted at `now`:
    /// listed, effective, not retired, and past its notice delay.
    ///
    /// # Errors
    ///
    /// Names why it is not.
    pub fn admits(&self, release_id: &str, release: &Release, now: u64) -> Result<(), String> {
        if release.workload != self.workload {
            return Err("the release is for another workload".into());
        }
        let entry = self
            .admitted
            .iter()
            .find(|entry| entry.release == release_id)
            .ok_or("the release is not in the head's admitted list")?;
        if entry.effective_at > now {
            return Err(format!(
                "the release is not effective until {}",
                entry.effective_at
            ));
        }
        if entry.retire_at.is_some_and(|retire| retire <= now) {
            return Err("the release is retired".into());
        }
        let emergency = self
            .emergency
            .as_ref()
            .is_some_and(|e| e.release == release_id);
        // An emergency shortens the notice, but never below a day.
        let notice = if emergency {
            self.notice_seconds.min(MIN_EMERGENCY_NOTICE_SECS)
        } else {
            self.notice_seconds
        };
        if entry.effective_at < release.published_at.saturating_add(notice) {
            return Err(format!(
                "the release took effect before its {notice}-second notice delay ended"
            ));
        }
        Ok(())
    }

    /// The head's address, `30202:<publisher>:<workload>`.
    #[must_use]
    pub fn address(&self, publisher: &str) -> String {
        format!("{HEAD_KIND}:{publisher}:{}", self.workload)
    }
}

/// Refuse a head whose generation is lower than one already seen
/// (rollback). Returns the generation to keep.
///
/// # Errors
///
/// When `head` rolls back.
pub fn check_generation(seen: Option<u64>, head: &Head) -> Result<u64, String> {
    match seen {
        Some(seen) if head.generation < seen => Err(format!(
            "the head's generation {} is lower than {seen}, already seen (rollback)",
            head.generation
        )),
        Some(seen) => Ok(seen.max(head.generation)),
        None => Ok(head.generation),
    }
}

impl Endpoint {
    /// Check the body on its own, including its binding.
    ///
    /// # Errors
    ///
    /// Names the first rule it breaks.
    pub fn validate(&self) -> Result<(), String> {
        if self.v != ENDPOINT_V {
            return Err("unknown endpoint version".into());
        }
        if !self.requires.is_empty() {
            return Err("the endpoint requires an unknown feature".into());
        }
        hex64(&self.release, "release")?;
        hex64(&self.endpoint, "endpoint")?;
        hex64(&self.operator, "operator")?;
        if self.evidence.is_empty() {
            return Err("an endpoint carries at least one piece of evidence".into());
        }
        if self.valid_until <= self.issued_at
            || self.valid_until > self.issued_at + MAX_ENDPOINT_VALIDITY_SECS
        {
            return Err("valid_until is not within 3600 seconds after issued_at".into());
        }
        let hpke = match &self.hpke {
            None => None,
            Some(text) => Some(base64_32(text).ok_or("hpke is not a base64 32-byte key")?),
        };
        if binding(&self.endpoint, hpke.as_ref(), &self.release)? != self.binding {
            return Err("the binding does not match the endpoint key and release".into());
        }
        Ok(())
    }

    /// Whether the endpoint is current at `now`.
    ///
    /// # Errors
    ///
    /// When it has expired or was issued in the future.
    pub fn current(&self, now: u64) -> Result<(), String> {
        if now >= self.valid_until {
            return Err("the endpoint has expired".into());
        }
        if self.issued_at > now + MAX_FUTURE_SKEW_SECS {
            return Err("the endpoint was issued in the future".into());
        }
        Ok(())
    }
}

/// Sign `release` as a `3202` event. The signer must be the publisher.
///
/// # Errors
///
/// When the body breaks a rule or the signer is not the publisher.
pub fn release_event(
    signer: &RelaySigner,
    release: &Release,
    created_at: u64,
) -> Result<Event, String> {
    release.validate()?;
    if signer.pubkey() != release.publisher {
        return Err("the release's publisher must sign it".into());
    }
    let content = canonical(release)?;
    let tags = vec![
        Tag::new(vec!["t".into(), RELEASE_MARKER.into()]),
        Tag::new(vec!["x".into(), sha256_hex(content.as_bytes())]),
        Tag::new(vec!["w".into(), release.workload.clone()]),
    ];
    Ok(signer.sign(created_at, RELEASE_KIND, tags, content))
}

/// Verify a `3202` event and return its body.
///
/// # Errors
///
/// Names the first refusal.
pub fn parse_release(event: &Event) -> Result<Release, String> {
    let release: Release = envelope(event, RELEASE_KIND, RELEASE_MARKER)?;
    release.validate()?;
    if release.publisher != event.pubkey {
        return Err("the release's publisher is not its signer".into());
    }
    if single_tag(event, "w")? != release.workload {
        return Err("`w` tag differs from the workload".into());
    }
    Ok(release)
}

/// Sign `head` as a `30202` event.
///
/// # Errors
///
/// When the body breaks a rule.
pub fn head_event(signer: &RelaySigner, head: &Head, created_at: u64) -> Result<Event, String> {
    head.validate()?;
    let content = canonical(head)?;
    let mut tags = vec![
        Tag::new(vec!["d".into(), head.workload.clone()]),
        Tag::new(vec!["t".into(), HEAD_MARKER.into()]),
        Tag::new(vec!["x".into(), sha256_hex(content.as_bytes())]),
    ];
    for entry in &head.admitted {
        tags.push(Tag::new(vec!["e".into(), entry.release.clone()]));
    }
    Ok(signer.sign(created_at, HEAD_KIND, tags, content))
}

/// Verify a `30202` event and return its body.
///
/// # Errors
///
/// Names the first refusal.
pub fn parse_head(event: &Event) -> Result<Head, String> {
    let head: Head = envelope(event, HEAD_KIND, HEAD_MARKER)?;
    head.validate()?;
    if single_tag(event, "d")? != head.workload {
        return Err("`d` tag differs from the workload".into());
    }
    let tagged: BTreeSet<&str> = event.tag_values("e").collect();
    let listed: BTreeSet<&str> = head.admitted.iter().map(|a| a.release.as_str()).collect();
    if tagged != listed {
        return Err("the `e` tags differ from the admitted releases".into());
    }
    Ok(head)
}

/// Sign `endpoint` as a `30203` event for instance `instance`, under the
/// head at `head_address`. The signer must be the endpoint key.
///
/// # Errors
///
/// When the body breaks a rule or the signer is not the endpoint key.
pub fn endpoint_event(
    signer: &RelaySigner,
    endpoint: &Endpoint,
    instance: &str,
    head_address: &str,
) -> Result<Event, String> {
    endpoint.validate()?;
    hex64(instance, "instance")?;
    if signer.pubkey() != endpoint.endpoint {
        return Err("the endpoint key must sign its endpoint".into());
    }
    let content = canonical(endpoint)?;
    let tags = vec![
        Tag::new(vec!["d".into(), instance.to_owned()]),
        Tag::new(vec!["t".into(), ENDPOINT_MARKER.into()]),
        Tag::new(vec!["x".into(), sha256_hex(content.as_bytes())]),
        Tag::new(vec!["a".into(), head_address.to_owned()]),
        Tag::new(vec!["e".into(), endpoint.release.clone()]),
        Tag::new(vec!["expiration".into(), endpoint.valid_until.to_string()]),
    ];
    Ok(signer.sign(endpoint.issued_at, ENDPOINT_KIND, tags, content))
}

/// Verify a `30203` event: signature, content, the signer is the endpoint
/// key, the binding, and the tags. Freshness is [`Endpoint::current`];
/// the evidence is the vendor verifier's.
///
/// # Errors
///
/// Names the first refusal.
pub fn parse_endpoint(event: &Event) -> Result<EndpointRecord, String> {
    let body: Endpoint = envelope(event, ENDPOINT_KIND, ENDPOINT_MARKER)?;
    if body.endpoint != event.pubkey {
        return Err("the endpoint key is not the event's signer".into());
    }
    body.validate()?;
    let instance = single_tag(event, "d")?.to_owned();
    hex64(&instance, "instance")?;
    if single_tag(event, "e")? != body.release {
        return Err("`e` tag differs from the release".into());
    }
    if single_tag(event, "expiration")? != body.valid_until.to_string() {
        return Err("expiration differs from valid_until".into());
    }
    let head = single_tag(event, "a")?.to_owned();
    if !head.starts_with(&format!("{HEAD_KIND}:")) {
        return Err("`a` tag is not a release head address".into());
    }
    Ok(EndpointRecord {
        body,
        instance,
        head,
    })
}

/// The fields a sealed decision request adds to its payload: `requires`
/// naming [`ATTESTED_FEATURE`] and `attested`, the client's view of the
/// endpoint at send time.
#[must_use]
pub fn sealed_fields(endpoint_address: &str, release: &str, level: Level) -> (Value, Value) {
    (
        json!([ATTESTED_FEATURE]),
        json!({"endpoint": endpoint_address, "release": release, "level": level.as_str()}),
    )
}

/// The worker's check of a decrypted request payload: it requires the
/// attested feature and names this endpoint and release. Returns the level
/// the client asked at.
///
/// # Errors
///
/// When the payload is not a sealed job for this endpoint and release.
pub fn check_sealed(
    payload: &Value,
    endpoint_address: &str,
    release: &str,
) -> Result<Level, String> {
    let requires = payload
        .get("requires")
        .and_then(Value::as_array)
        .ok_or("the request does not require the attested feature")?;
    if !requires.iter().any(|f| f == ATTESTED_FEATURE) {
        return Err("the request does not require the attested feature".into());
    }
    let attested = payload
        .get("attested")
        .ok_or("the request names no attested endpoint")?;
    if attested.get("endpoint").and_then(Value::as_str) != Some(endpoint_address) {
        return Err("the request names another endpoint".into());
    }
    if attested.get("release").and_then(Value::as_str) != Some(release) {
        return Err("the request names another release".into());
    }
    attested
        .get("level")
        .and_then(Value::as_str)
        .and_then(Level::parse)
        .ok_or_else(|| "the request names no known level".into())
}

/// The attestation block an attested worker adds to a sealed answer's
/// response (`response.attested`): which endpoint, release and measurement
/// answered, at what level, for which request ciphertext, with which model.
/// The receipt's `result_digest` ([`response_digest`]) covers it.
#[must_use]
pub fn attested_block(
    endpoint_address: &str,
    release: &str,
    level: Level,
    measurement: &str,
    request_ciphertext: &str,
    model: &str,
    model_digest: &str,
) -> Value {
    json!({
        "endpoint": endpoint_address,
        "release": release,
        "level": level.as_str(),
        "measurement": measurement,
        "request_ciphertext_digest": sha256_hex(request_ciphertext.as_bytes()),
        "model": model,
        "model_digest": model_digest,
    })
}

/// The digest a sealed answer's receipt records for its response: SHA-256
/// of the response's JCS bytes, so caller and worker agree whatever their
/// JSON map order.
#[must_use]
pub fn response_digest(response: &Value) -> String {
    sha256_hex(&jcs(response).unwrap_or_default())
}

/// The lowercase SHA-256 hex digest of `bytes`.
#[must_use]
pub fn sha256_hex(bytes: &[u8]) -> String {
    to_hex(&Sha256::digest(bytes))
}

fn to_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn canonical<T: Serialize>(body: &T) -> Result<String, String> {
    let value = serde_json::to_value(body).map_err(|e| e.to_string())?;
    String::from_utf8(jcs(&value).map_err(|e| e.to_string())?).map_err(|e| e.to_string())
}

fn is_lower_hex(value: &str) -> bool {
    value
        .bytes()
        .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

fn hex64(value: &str, field: &str) -> Result<(), String> {
    if value.len() == 64 && is_lower_hex(value) {
        Ok(())
    } else {
        Err(format!("{field} is not 64 lowercase hex characters"))
    }
}

fn hex32(value: &str, field: &str) -> Result<[u8; 32], String> {
    hex64(value, field)?;
    let mut out = [0u8; 32];
    for (i, chunk) in value.as_bytes().chunks(2).enumerate() {
        let text = std::str::from_utf8(chunk).map_err(|e| e.to_string())?;
        out[i] = u8::from_str_radix(text, 16).map_err(|e| e.to_string())?;
    }
    Ok(out)
}

fn sha256_ref(value: &str, field: &str) -> Result<(), String> {
    match value.strip_prefix("sha256:") {
        Some(hex) if hex.len() == 64 && is_lower_hex(hex) => Ok(()),
        _ => Err(format!("{field} is not sha256:<64 lowercase hex>")),
    }
}

fn slug(value: &str, field: &str) -> Result<(), String> {
    if !value.is_empty()
        && value.len() <= 64
        && value
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_' || b == b'-')
    {
        Ok(())
    } else {
        Err(format!("{field} is not a slug of 1 to 64 [a-z0-9_-] bytes"))
    }
}

/// Standard base64 (with or without padding) of exactly 32 bytes.
fn base64_32(text: &str) -> Option<[u8; 32]> {
    const ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let trimmed = text.trim_end_matches('=');
    let mut bits: u32 = 0;
    let mut count = 0u32;
    let mut out = Vec::with_capacity(33);
    for byte in trimmed.bytes() {
        let value = ALPHABET.iter().position(|&a| a == byte)? as u32;
        bits = (bits << 6) | value;
        count += 6;
        if count >= 8 {
            count -= 8;
            out.push((bits >> count) as u8);
            bits &= (1 << count) - 1;
        }
    }
    out.try_into().ok()
}

fn single_tag<'a>(event: &'a Event, name: &'a str) -> Result<&'a str, String> {
    let mut values = event.tag_values(name);
    let first = values
        .next()
        .ok_or_else(|| format!("missing `{name}` tag"))?;
    if values.next().is_some() {
        return Err(format!("more than one `{name}` tag"));
    }
    Ok(first)
}

/// What every NIP-ATT event shares: kind, signature, the single `t`
/// marker, the `x` digest of the exact content, and JCS content.
fn envelope<T: for<'de> Deserialize<'de> + Serialize>(
    event: &Event,
    kind: u16,
    marker: &str,
) -> Result<T, String> {
    if event.kind != kind {
        return Err(format!("expected kind {kind}, got {}", event.kind));
    }
    event
        .validate_crypto()
        .map_err(|e| format!("bad signature: {e}"))?;
    if single_tag(event, "t")? != marker {
        return Err("wrong `t` marker".into());
    }
    if single_tag(event, "x")? != sha256_hex(event.content.as_bytes()) {
        return Err("`x` tag does not match the content".into());
    }
    let body: T = serde_json::from_str(&event.content).map_err(|e| format!("bad body: {e}"))?;
    if canonical(&body)?.as_bytes() != event.content.as_bytes() {
        return Err("content is not the canonical JSON of its body".into());
    }
    Ok(body)
}

#[cfg(test)]
mod tests;
