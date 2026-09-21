//! The manifest document: schema, validation, and digest.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};

/// The schema tag a manifest carries. A file that names another version is
/// refused rather than read partially.
pub const SCHEMA: &str = "openagents.tenancy.v1";

/// The lane a binding serves on.
///
/// The lane is a property of the binding, not the door: the same model can
/// sit behind a shared door for every keyed tenant and a dedicated door
/// for one, and the two bindings are independent authorizations.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Lane {
    /// A stock door every keyed tenant may reach.
    Shared,
    /// A door bound to this tenant alone.
    Dedicated,
    /// A dedicated door serving a tenant-trained adapter. The binding must
    /// name the admission record that authorized the candidate.
    Trained,
    /// The caller's own device. The registry can record the expectation,
    /// though the host never sees the call.
    OnDevice,
}

/// The artifact a bound door is expected to be serving.
///
/// `artifact_signature` is the content digest the serving process must
/// publish — `sha256:` followed by 64 lowercase hex characters. An empty
/// signature means the binding pins a name and nothing more: the right
/// shape for a hosted closed model, and a label rather than a proof.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Expected {
    /// The model id the door must report.
    pub model: String,
    /// The adapter package the door must serve, when the binding names one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub adapter: Option<String>,
    /// The content digest the door must publish, or empty to leave the
    /// artifact unpinned.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub artifact_signature: String,
    /// The execution settings the door must publish — dtype, backend, and
    /// the other numerical choices an accuracy was measured under. Every
    /// bound key must match what the door reports; a drifted setting is a
    /// fault the same way a swapped artifact is.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub execution: BTreeMap<String, String>,
}

/// The capacity a binding assigns, when the agreement names one.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct Capacity {
    /// How many calls the door may run for this binding at once.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub concurrency: Option<u64>,
    /// The sustained call rate the binding allows.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub requests_per_minute: Option<u64>,
}

/// One door a tenant may reach, and the identity it must serve under.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Binding {
    /// The lane this binding serves on.
    pub lane: Lane,
    /// The artifact identity the door must publish.
    pub artifact: Expected,
    /// The capacity the agreement assigns, when it assigns one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub capacity: Option<Capacity>,
    /// The admission record that authorized a trained artifact. Required
    /// on a `trained`-lane binding; meaningless elsewhere. The record
    /// itself is the admission contract's business — the binding only
    /// proves a reference was named.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub promotion: Option<String>,
}

/// A tenant's budget, when the agreement names one.
///
/// The units are resources, not money: requests, questions, input bytes,
/// and how many calls may be outstanding at once. A quota is not a price —
/// what a unit costs is the pricing contract's business, and this field
/// says nothing about it.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct Quota {
    /// Requests admitted per UTC day.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub requests_per_day: Option<u64>,
    /// Questions answered per UTC day.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub questions_per_day: Option<u64>,
    /// Input bytes admitted per UTC day — a question over a long state is
    /// not the same work as a short one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub input_bytes_per_day: Option<u64>,
    /// Reservations a tenant may hold at once.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub concurrency: Option<u64>,
    /// The settlement policy's name — which outcomes count against the
    /// budget. The quota module implements `quota-v1`; anything else is
    /// refused rather than guessed at.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub policy: Option<String>,
}

/// A tenant: a stable identity, a credential reference, and its doors.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Tenant {
    /// A reference to the tenant's credential — a key id or its digest.
    /// The secret itself never appears in this document: the
    /// authentication layer resolves the reference.
    pub credential: String,
    /// Authenticated external principals that resolve to this tenant — a
    /// relay's NIP-42 pubkey as `nostr:<hex>`, for instance. An HTTP key
    /// and a relay principal land on the same record, so one binding
    /// decides both transports.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub principals: Vec<String>,
    /// The doors this tenant may reach beyond the shared set.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub doors: BTreeMap<String, Binding>,
    /// The tenant's budget, when the agreement names one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub quota: Option<Quota>,
}

/// The manifest: every tenant, every shared door, one digest.
///
/// `digest` covers every field but itself, canonicalized key-sorted JSON
/// over SHA-256, so a file that cannot recompute its own digest is refused
/// before it is read. `supersedes` names the digest of the revision this
/// one replaced — the manifest chain, not any row's.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Manifest {
    /// The schema tag.
    pub v: String,
    /// The revision number. Genesis is 0; each update adds one.
    pub sequence: u64,
    /// The digest of the revision this one replaced, when there was one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub supersedes: Option<String>,
    /// The stock doors every keyed tenant may reach.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub shared: BTreeMap<String, Binding>,
    /// The tenants and the doors bound to them alone.
    pub tenants: BTreeMap<String, Tenant>,
    /// The digest over every field above.
    pub digest: String,
}

impl Manifest {
    /// Fill in `digest` over the manifest's other fields.
    pub fn seal(&mut self) {
        self.digest = self.compute_digest();
    }

    /// The digest over every field but `digest`.
    #[must_use]
    pub fn compute_digest(&self) -> String {
        let mut value = serde_json::to_value(self).expect("a manifest serializes");
        value
            .as_object_mut()
            .expect("a manifest is an object")
            .remove("digest");
        let mut hasher = Sha256::new();
        hasher.update(canonicalize(&value).as_bytes());
        format!("sha256:{:x}", hasher.finalize())
    }

    /// Read and validate a manifest's text.
    ///
    /// Refused: a schema tag this crate does not know, a digest that does
    /// not recompute over the contents, a malformed artifact signature, a
    /// `shared` binding on any lane but shared, a `trained` lane without
    /// its promotion reference, or a tenant door that shares a name with a
    /// shared binding. The last is refused rather than shadowed: which
    /// binding applies should never be a question.
    pub fn parse(text: &str, name: &str) -> Result<Self, String> {
        let manifest: Self =
            serde_json::from_str(text).map_err(|error| format!("{name}: {error}"))?;
        manifest.validate(name)?;
        Ok(manifest)
    }

    /// The checks [`Manifest::parse`] runs, separated so a freshly built
    /// manifest can be validated before it is installed.
    pub fn validate(&self, name: &str) -> Result<(), String> {
        if self.v != SCHEMA {
            return Err(format!("{name}: schema `{}` is not `{SCHEMA}`", self.v));
        }
        if self.digest != self.compute_digest() {
            return Err(format!(
                "{name}: the manifest's digest does not recompute over its contents"
            ));
        }
        for (door, binding) in &self.shared {
            validate_binding(name, door, binding)?;
            if binding.lane != Lane::Shared {
                return Err(format!(
                    "{name}: shared door `{door}` binds a {} lane",
                    lane_name(binding.lane)
                ));
            }
        }
        for (tenant, record) in &self.tenants {
            if record.credential.is_empty() {
                return Err(format!(
                    "{name}: tenant `{tenant}` carries no credential reference"
                ));
            }
            for (door, binding) in &record.doors {
                validate_binding(name, door, binding)?;
                if self.shared.contains_key(door) {
                    return Err(format!(
                        "{name}: tenant `{tenant}` binds `{door}`, which is a shared door — \
                         a door is bound to one tenant or to all, never both"
                    ));
                }
                if binding.lane == Lane::Shared {
                    return Err(format!(
                        "{name}: tenant `{tenant}` binds `{door}` to the shared lane — \
                         a shared door belongs in the manifest's `shared` section"
                    ));
                }
            }
        }
        Ok(())
    }
}

/// The checks a binding must pass wherever it appears.
fn validate_binding(name: &str, door: &str, binding: &Binding) -> Result<(), String> {
    if !binding.artifact.artifact_signature.is_empty()
        && !binding
            .artifact
            .artifact_signature
            .strip_prefix("sha256:")
            .is_some_and(|digest| {
                digest.len() == 64 && digest.bytes().all(|byte| byte.is_ascii_hexdigit())
            })
    {
        return Err(format!(
            "{name}: door `{door}` pins artifact signature `{}`, which is not \
             `sha256:` followed by 64 hex characters",
            binding.artifact.artifact_signature
        ));
    }
    if binding.lane == Lane::Trained && binding.promotion.as_deref().is_none_or(str::is_empty) {
        return Err(format!(
            "{name}: door `{door}` is a trained lane without a promotion reference — \
             an admitted candidate names the record that admitted it"
        ));
    }
    Ok(())
}

/// A lane's wire label, for errors and reports.
pub fn lane_name(lane: Lane) -> &'static str {
    match lane {
        Lane::Shared => "shared",
        Lane::Dedicated => "dedicated",
        Lane::Trained => "trained",
        Lane::OnDevice => "on-device",
    }
}

/// Canonical JSON: keys sorted, whitespace gone, so two writers digest the
/// same content to the same bytes. The same canonicalization the suite
/// digests use. The keys are sorted here rather than trusted to the map:
/// `preserve_order` makes a `serde_json` map insertion-ordered whenever a
/// sibling crate enables it, and the digest agreement must not depend on
/// who wrote the bytes.
fn canonicalize(value: &Value) -> String {
    match value {
        Value::Object(map) => {
            let mut keys: Vec<&String> = map.keys().collect();
            keys.sort();
            let mut out = String::from("{");
            for (index, key) in keys.iter().enumerate() {
                if index > 0 {
                    out.push(',');
                }
                out.push_str(&serde_json::to_string(key).expect("a key serializes"));
                out.push(':');
                out.push_str(&canonicalize(&map[*key]));
            }
            out.push('}');
            out
        }
        Value::Array(items) => {
            let mut out = String::from("[");
            for (index, item) in items.iter().enumerate() {
                if index > 0 {
                    out.push(',');
                }
                out.push_str(&canonicalize(item));
            }
            out.push(']');
            out
        }
        other => serde_json::to_string(other).expect("a value serializes"),
    }
}
