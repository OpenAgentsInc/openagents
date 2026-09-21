//! The registry on disk: install, open, update, authorize.

use std::collections::BTreeMap;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::manifest::{Binding, Manifest};

/// The file the current manifest lives in.
const CURRENT: &str = "registry.json";

/// The directory past revisions are archived in, one file per digest.
const HISTORY: &str = "history";

/// What went wrong reading or writing a registry.
#[derive(Debug)]
pub enum Trouble {
    /// The filesystem refused.
    Io(std::io::Error),
    /// The manifest failed validation; the message names why.
    Invalid(String),
    /// An update named a `supersedes` that is not the installed digest —
    /// the writer was looking at a different registry than the one on
    /// disk.
    Stale { expected: String, found: String },
    /// An update's sequence did not follow the installed one.
    Sequence { expected: u64, found: u64 },
    /// A history lookup named a digest no archived revision carries.
    UnknownRevision(String),
}

impl std::fmt::Display for Trouble {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(error) => write!(f, "{error}"),
            Self::Invalid(message) => write!(f, "{message}"),
            Self::Stale { expected, found } => write!(
                f,
                "the update supersedes `{expected}`, but the installed registry is `{found}` — \
                 the writer was not looking at the current revision"
            ),
            Self::Sequence { expected, found } => write!(
                f,
                "the update is sequence {found}, but sequence {expected} follows the installed \
                 revision"
            ),
            Self::UnknownRevision(digest) => {
                write!(f, "no archived revision carries digest `{digest}`")
            }
        }
    }
}

impl std::error::Error for Trouble {}

impl From<std::io::Error> for Trouble {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error)
    }
}

/// Why a request was refused before it reached a door.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Refusal {
    /// The tenant is not in the registry.
    UnknownTenant(String),
    /// The tenant is known but holds no binding for this door, and the
    /// door is not shared.
    DoorNotBound { tenant: String, door: String },
    /// An anonymous request asked for a door the shared lane does not
    /// carry.
    NotShared(String),
}

impl std::fmt::Display for Refusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnknownTenant(tenant) => {
                write!(f, "`{tenant}` is not a tenant this registry knows")
            }
            Self::DoorNotBound { tenant, door } => write!(
                f,
                "tenant `{tenant}` holds no binding for `{door}`, and `{door}` is not shared"
            ),
            Self::NotShared(door) => {
                write!(f, "`{door}` is not a shared door")
            }
        }
    }
}

/// How a published identity disagrees with the binding it was admitted
/// under.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Fault {
    /// The door reports a different model id.
    ModelMoved { expected: String, published: String },
    /// The door reports a different adapter, or none where one is bound.
    AdapterMoved {
        expected: Option<String>,
        published: String,
    },
    /// The door's content digest is not the bound one — an artifact was
    /// replaced under a name the registry still names.
    ArtifactSwapped { expected: String, published: String },
    /// A bound execution setting does not match what the door publishes.
    ExecutionDrift {
        key: String,
        expected: String,
        published: Option<String>,
    },
}

impl std::fmt::Display for Fault {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ModelMoved {
                expected,
                published,
            } => write!(
                f,
                "the door publishes model `{published}`; the binding names `{expected}`"
            ),
            Self::AdapterMoved {
                expected,
                published,
            } => write!(
                f,
                "the door publishes adapter `{published}`; the binding names {}",
                expected.as_deref().map_or("none", |name| name)
            ),
            Self::ArtifactSwapped {
                expected,
                published,
            } => write!(
                f,
                "the door publishes artifact `{published}`; the binding pins `{expected}` — \
                 a replaced artifact under a known name is refused until a registry \
                 update authorizes it"
            ),
            Self::ExecutionDrift {
                key,
                expected,
                published,
            } => write!(
                f,
                "the door publishes {key}={}; the binding binds {key}={expected}",
                published.as_deref().unwrap_or("nothing"),
            ),
        }
    }
}

/// The identity a serving process publishes — the fields of its model
/// card that a binding can check.
///
/// This is the serving process's claim, the same claim a gym row records
/// as `door_identity`. It is not attestation: `verify` proves the process
/// says what the registry bound, never that the weights are what the
/// process believes.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Published {
    /// The model id the door reports.
    pub model: String,
    /// The adapter package the door reports, or empty.
    pub adapter: String,
    /// The content digest the door reports, or empty when it publishes
    /// none.
    pub artifact_signature: String,
    /// The execution settings the door reports.
    pub execution: BTreeMap<String, String>,
}

/// The binding a request was admitted under — a snapshot the request keeps
/// for its whole flight.
///
/// An admission is a copy, not a reference: a registry update after
/// admission cannot relabel a call in flight. The receipt for the call
/// names `registry_digest` and `sequence`, so which revision authorized it
/// is a lookup, not a guess.
#[derive(Clone, Debug)]
pub struct Admission {
    /// The tenant the request was admitted as.
    pub tenant: Option<String>,
    /// The door the binding names.
    pub door: String,
    /// The binding as it stood at admission.
    pub binding: Binding,
    /// The registry digest that authorized this admission.
    pub registry_digest: String,
    /// The registry sequence that authorized this admission.
    pub sequence: u64,
}

impl Admission {
    /// Check a published identity against the bound expectation.
    ///
    /// Every bound field must match: the model id, the adapter when one is
    /// bound, the artifact signature when one is pinned, and every bound
    /// execution key. A binding that pins no artifact signature can only
    /// check the model name — `verified` reports which case this is.
    pub fn verify(&self, published: &Published) -> Result<(), Fault> {
        let expected = &self.binding.artifact;
        if published.model != expected.model {
            return Err(Fault::ModelMoved {
                expected: expected.model.clone(),
                published: published.model.clone(),
            });
        }
        let bound_adapter = expected.adapter.clone().unwrap_or_default();
        if published.adapter != bound_adapter {
            return Err(Fault::AdapterMoved {
                expected: expected.adapter.clone(),
                published: published.adapter.clone(),
            });
        }
        if !expected.artifact_signature.is_empty()
            && published.artifact_signature != expected.artifact_signature
        {
            return Err(Fault::ArtifactSwapped {
                expected: expected.artifact_signature.clone(),
                published: published.artifact_signature.clone(),
            });
        }
        for (key, value) in &expected.execution {
            let found = published.execution.get(key);
            if found != Some(value) {
                return Err(Fault::ExecutionDrift {
                    key: key.clone(),
                    expected: value.clone(),
                    published: found.cloned(),
                });
            }
        }
        Ok(())
    }

    /// Whether this binding pins a content digest. An unpinned binding's
    /// `verify` is a name check only — the `verified: false` rule applied
    /// to a binding.
    #[must_use]
    pub fn pins_artifact(&self) -> bool {
        !self.binding.artifact.artifact_signature.is_empty()
    }

    /// Whether the registry still binds what this admission carries — the
    /// same door under the same artifact and settings. An in-flight call
    /// does not need this: it keeps the identity it was admitted under.
    /// A caller wondering whether its admission is stale does.
    #[must_use]
    pub fn is_current(&self, registry: &Registry) -> bool {
        registry
            .authorize(self.tenant.as_deref(), &self.door)
            .is_ok_and(|fresh| {
                fresh.registry_digest == self.registry_digest && fresh.binding == self.binding
            })
    }
}

/// One line of the revision log: which manifest followed which.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Revision {
    /// The revision's sequence number.
    pub sequence: u64,
    /// The revision's digest.
    pub digest: String,
    /// The digest it replaced, when it replaced one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub supersedes: Option<String>,
    /// When the revision was installed, as RFC 3339 in UTC.
    pub installed_at: String,
}

/// A registry directory: the current manifest and its history.
#[derive(Debug)]
pub struct Registry {
    manifest: Manifest,
}

impl Registry {
    /// Install a registry's first manifest in a directory.
    ///
    /// The manifest must be sequence 0 and supersede nothing — a registry
    /// begins, it does not appear mid-chain. The manifest is sealed
    /// (digest computed) if it is not already, validated, archived into
    /// `history/`, and written as `registry.json`.
    pub fn install(dir: &Path, mut manifest: Manifest) -> Result<Self, Trouble> {
        if manifest.sequence != 0 {
            return Err(Trouble::Sequence {
                expected: 0,
                found: manifest.sequence,
            });
        }
        if manifest.supersedes.is_some() {
            return Err(Trouble::Invalid(
                "a registry's first manifest supersedes nothing".to_string(),
            ));
        }
        if dir.join(CURRENT).exists() {
            return Err(Trouble::Invalid(format!(
                "{} already holds a registry; update it rather than reinstalling",
                dir.display()
            )));
        }
        manifest.seal();
        manifest
            .validate(&dir.join(CURRENT).display().to_string())
            .map_err(Trouble::Invalid)?;
        Self::write(dir, manifest)
    }

    /// Open the installed registry, validating its manifest.
    pub fn open(dir: &Path) -> Result<Self, Trouble> {
        let path = dir.join(CURRENT);
        let text = std::fs::read_to_string(&path)?;
        let manifest =
            Manifest::parse(&text, &path.display().to_string()).map_err(Trouble::Invalid)?;
        Ok(Self { manifest })
    }

    /// Replace the installed manifest with the next revision.
    ///
    /// The new manifest must carry `sequence` one past the installed
    /// revision and `supersedes` equal to the installed digest — an update
    /// written against a stale view is refused rather than silently
    /// winning. The write is a temp file and a rename, so a reader sees
    /// the old manifest or the new one, never a torn one. The old manifest
    /// stays archived under its digest; the new one is archived too, and
    /// the revision log gains a line.
    pub fn update(dir: &Path, mut manifest: Manifest) -> Result<Self, Trouble> {
        let installed = Self::open(dir)?;
        if manifest.sequence != installed.manifest.sequence + 1 {
            return Err(Trouble::Sequence {
                expected: installed.manifest.sequence + 1,
                found: manifest.sequence,
            });
        }
        match manifest.supersedes.as_deref() {
            Some(supersedes) if supersedes == installed.manifest.digest => {}
            other => {
                return Err(Trouble::Stale {
                    expected: other.unwrap_or("nothing").to_string(),
                    found: installed.manifest.digest.clone(),
                });
            }
        }
        manifest.seal();
        manifest
            .validate(&dir.join(CURRENT).display().to_string())
            .map_err(Trouble::Invalid)?;
        Self::write(dir, manifest)
    }

    /// Read an archived revision by digest.
    ///
    /// The history is how a request answered yesterday gets explained
    /// today: the receipt names the registry digest it was admitted under,
    /// and this is the lookup that digest resolves through.
    pub fn revision(dir: &Path, digest: &str) -> Result<Manifest, Trouble> {
        let path = dir.join(HISTORY).join(format!("{digest}.json"));
        if !path.exists() {
            return Err(Trouble::UnknownRevision(digest.to_string()));
        }
        let text = std::fs::read_to_string(&path)?;
        Manifest::parse(&text, &path.display().to_string()).map_err(Trouble::Invalid)
    }

    /// The installed manifest.
    #[must_use]
    pub fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    /// The installed revision's digest.
    #[must_use]
    pub fn digest(&self) -> &str {
        &self.manifest.digest
    }

    /// The installed revision's sequence.
    #[must_use]
    pub fn sequence(&self) -> u64 {
        self.manifest.sequence
    }

    /// Whether a request may proceed, and under which binding.
    ///
    /// `tenant` is `None` for an anonymous call, which can only reach the
    /// shared set. A keyed tenant's own bindings are checked first, then
    /// the shared set — a door bound to another tenant is as unreachable
    /// as one the registry never named, because the lookup is under the
    /// caller's own tenant record, not the door's.
    pub fn authorize(&self, tenant: Option<&str>, door: &str) -> Result<Admission, Refusal> {
        match tenant {
            Some(tenant) => {
                let record = self
                    .manifest
                    .tenants
                    .get(tenant)
                    .ok_or_else(|| Refusal::UnknownTenant(tenant.to_string()))?;
                let binding = record
                    .doors
                    .get(door)
                    .or_else(|| self.manifest.shared.get(door))
                    .ok_or_else(|| Refusal::DoorNotBound {
                        tenant: tenant.to_string(),
                        door: door.to_string(),
                    })?;
                Ok(self.admission(Some(tenant), door, binding))
            }
            None => {
                let binding = self
                    .manifest
                    .shared
                    .get(door)
                    .ok_or_else(|| Refusal::NotShared(door.to_string()))?;
                Ok(self.admission(None, door, binding))
            }
        }
    }

    /// The tenant an authenticated external principal belongs to — a
    /// relay's NIP-42 pubkey resolving to the same record an HTTP key
    /// would. Absent principals name no tenant.
    #[must_use]
    pub fn tenant_of_principal(&self, principal: &str) -> Option<&str> {
        self.manifest
            .tenants
            .iter()
            .find(|(_, record)| record.principals.iter().any(|p| p == principal))
            .map(|(tenant, _)| tenant.as_str())
    }

    /// The doors a tenant may name, for discovery: their own bindings plus
    /// the shared set. Discovery reads the same map authorization does —
    /// a tenant cannot learn a door exists by asking for a catalog either.
    #[must_use]
    pub fn visible_doors(&self, tenant: &str) -> Vec<String> {
        let mut doors: Vec<String> = self
            .manifest
            .tenants
            .get(tenant)
            .map(|record| record.doors.keys().cloned().collect())
            .unwrap_or_default();
        doors.extend(self.manifest.shared.keys().cloned());
        doors.sort();
        doors.dedup();
        doors
    }

    /// Snapshot an admission for `tenant` over `binding`.
    fn admission(&self, tenant: Option<&str>, door: &str, binding: &Binding) -> Admission {
        Admission {
            tenant: tenant.map(str::to_string),
            door: door.to_string(),
            binding: binding.clone(),
            registry_digest: self.manifest.digest.clone(),
            sequence: self.manifest.sequence,
        }
    }

    /// Write a manifest: archive it by digest, replace `registry.json`
    /// atomically, and append the revision line.
    fn write(dir: &Path, manifest: Manifest) -> Result<Self, Trouble> {
        let history = dir.join(HISTORY);
        std::fs::create_dir_all(&history)?;
        let archived = history.join(format!("{}.json", manifest.digest));
        let text = serde_json::to_string_pretty(&manifest)
            .map_err(|error| Trouble::Invalid(error.to_string()))?;
        if !archived.exists() {
            std::fs::write(&archived, format!("{text}\n"))?;
        }

        let current = dir.join(CURRENT);
        let staged = dir.join(format!(".{CURRENT}.tmp"));
        std::fs::write(&staged, format!("{text}\n"))?;
        std::fs::rename(&staged, &current)?;

        let line = Revision {
            sequence: manifest.sequence,
            digest: manifest.digest.clone(),
            supersedes: manifest.supersedes.clone(),
            installed_at: now_utc(),
        };
        let mut log = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(dir.join("revisions.jsonl"))?;
        use std::io::Write;
        writeln!(
            log,
            "{}",
            serde_json::to_string(&line).map_err(|error| Trouble::Invalid(error.to_string()))?
        )?;

        Ok(Self { manifest })
    }
}

/// The current UTC time, as RFC 3339. The registry needs a timestamp for
/// its revision log and carries no clock dependency for one.
pub(crate) fn now_utc() -> String {
    let seconds = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|span| span.as_secs())
        .unwrap_or_default();
    let days = seconds / 86400;
    let day_seconds = seconds % 86400;
    let (year, month, day) = civil_from_days(days as i64);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z",
        day_seconds / 3600,
        (day_seconds % 3600) / 60,
        day_seconds % 60
    )
}

/// Days since the epoch to a calendar date, by Howard Hinnant's algorithm.
fn civil_from_days(days: i64) -> (i64, u64, u64) {
    let days = days + 719_468;
    let era = if days >= 0 { days } else { days - 146_096 } / 146_097;
    let day_of_era = (days - era * 146_097) as u64;
    let year_of_era =
        (day_of_era - day_of_era / 1460 + day_of_era / 36524 - day_of_era / 146_096) / 365;
    let year = year_of_era as i64 + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_prime = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_prime + 2) / 5 + 1;
    let month = if month_prime < 10 {
        month_prime + 3
    } else {
        month_prime - 9
    };
    (if month <= 2 { year + 1 } else { year }, month, day)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::manifest::{Capacity, Expected, Lane, Tenant};

    fn digest_of(byte: char) -> String {
        format!("sha256:{}", byte.to_string().repeat(64))
    }

    fn shared_binding(signature: &str) -> Binding {
        Binding {
            lane: Lane::Shared,
            artifact: Expected {
                model: "kev-0.6b".to_string(),
                adapter: None,
                artifact_signature: signature.to_string(),
                execution: [("dtype".to_string(), "bf16".to_string())]
                    .into_iter()
                    .collect(),
            },
            capacity: Some(Capacity {
                concurrency: Some(4),
                requests_per_minute: Some(600),
            }),
            promotion: None,
        }
    }

    fn dedicated_binding(signature: &str) -> Binding {
        Binding {
            lane: Lane::Dedicated,
            artifact: Expected {
                model: "kev-4b".to_string(),
                adapter: None,
                artifact_signature: signature.to_string(),
                execution: [("dtype".to_string(), "bf16".to_string())]
                    .into_iter()
                    .collect(),
            },
            capacity: None,
            promotion: None,
        }
    }

    fn manifest(sequence: u64, supersedes: Option<String>) -> Manifest {
        let mut tenants = BTreeMap::new();
        tenants.insert(
            "acme".to_string(),
            Tenant {
                credential: "key-ref:acme/2026-09".to_string(),
                principals: vec!["nostr:acme-pubkey".to_string()],
                doors: [(
                    "acme-dedicated".to_string(),
                    dedicated_binding(&digest_of('b')),
                )]
                .into_iter()
                .collect(),
                quota: None,
            },
        );
        tenants.insert(
            "globex".to_string(),
            Tenant {
                credential: "key-ref:globex/2026-09".to_string(),
                principals: vec![],
                doors: BTreeMap::new(),
                quota: None,
            },
        );
        Manifest {
            v: crate::SCHEMA.to_string(),
            sequence,
            supersedes,
            shared: [("kev-0.6b".to_string(), shared_binding(&digest_of('a')))]
                .into_iter()
                .collect(),
            tenants,
            digest: String::new(),
        }
    }

    fn published(signature: &str) -> Published {
        Published {
            model: "kev-0.6b".to_string(),
            adapter: String::new(),
            artifact_signature: signature.to_string(),
            execution: [("dtype".to_string(), "bf16".to_string())]
                .into_iter()
                .collect(),
        }
    }

    fn installed() -> (tempfile::TempDir, Registry) {
        let dir = tempfile::tempdir().unwrap();
        let registry = Registry::install(dir.path(), manifest(0, None)).unwrap();
        (dir, registry)
    }

    #[test]
    fn a_tenant_reaches_its_own_and_shared_doors_only() {
        let (_dir, registry) = installed();
        assert!(registry.authorize(Some("acme"), "acme-dedicated").is_ok());
        assert!(registry.authorize(Some("acme"), "kev-0.6b").is_ok());
        assert!(registry.authorize(Some("globex"), "kev-0.6b").is_ok());
        assert!(matches!(
            registry.authorize(Some("globex"), "acme-dedicated"),
            Err(Refusal::DoorNotBound { .. })
        ));
        assert!(matches!(
            registry.authorize(Some("acme"), "never-heard-of-it"),
            Err(Refusal::DoorNotBound { .. })
        ));
        assert!(matches!(
            registry.authorize(Some("initech"), "kev-0.6b"),
            Err(Refusal::UnknownTenant(_))
        ));
        assert!(matches!(
            registry.authorize(None, "acme-dedicated"),
            Err(Refusal::NotShared(_))
        ));
        assert!(registry.authorize(None, "kev-0.6b").is_ok());
    }

    #[test]
    fn a_relay_principal_resolves_to_the_same_tenant() {
        let (_dir, registry) = installed();
        assert_eq!(
            registry.tenant_of_principal("nostr:acme-pubkey"),
            Some("acme")
        );
        assert_eq!(registry.tenant_of_principal("nostr:nobody"), None);
        // The resolved tenant authorizes exactly as a keyed caller would.
        let tenant = registry.tenant_of_principal("nostr:acme-pubkey").unwrap();
        assert!(registry.authorize(Some(tenant), "acme-dedicated").is_ok());
    }

    #[test]
    fn a_swapped_artifact_is_refused_until_the_registry_says_otherwise() {
        let (dir, registry) = installed();
        let admission = registry.authorize(Some("acme"), "kev-0.6b").unwrap();
        assert!(matches!(
            admission.verify(&published(&digest_of('f'))),
            Err(Fault::ArtifactSwapped { .. })
        ));

        // The operator authorizes the replacement explicitly.
        let mut next = manifest(1, Some(registry.digest().to_string()));
        next.shared
            .get_mut("kev-0.6b")
            .unwrap()
            .artifact
            .artifact_signature = digest_of('f');
        let registry = Registry::update(dir.path(), next).unwrap();
        let admission = registry.authorize(Some("acme"), "kev-0.6b").unwrap();
        admission.verify(&published(&digest_of('f'))).unwrap();
    }

    #[test]
    fn a_drifted_execution_setting_is_a_fault() {
        let (_dir, registry) = installed();
        let admission = registry.authorize(Some("acme"), "kev-0.6b").unwrap();
        let mut drifted = published(&digest_of('a'));
        drifted
            .execution
            .insert("dtype".to_string(), "fp32".to_string());
        assert!(matches!(
            admission.verify(&drifted),
            Err(Fault::ExecutionDrift { .. })
        ));
        let mut missing = published(&digest_of('a'));
        missing.execution.clear();
        assert!(matches!(
            admission.verify(&missing),
            Err(Fault::ExecutionDrift { .. })
        ));
    }

    #[test]
    fn an_in_flight_call_keeps_the_identity_it_was_admitted_under() {
        let (dir, registry) = installed();
        let admission = registry.authorize(Some("acme"), "kev-0.6b").unwrap();
        let admitted_digest = admission.registry_digest.clone();

        let mut next = manifest(1, Some(registry.digest().to_string()));
        next.shared
            .get_mut("kev-0.6b")
            .unwrap()
            .artifact
            .artifact_signature = digest_of('f');
        let registry = Registry::update(dir.path(), next).unwrap();

        // The admission still names the artifact it was admitted under —
        // the update did not rewrite a call in flight.
        admission.verify(&published(&digest_of('a'))).unwrap();
        assert!(!admission.is_current(&registry));
        assert_eq!(admission.registry_digest, admitted_digest);
    }

    #[test]
    fn history_explains_an_earlier_revision_without_rewriting_it() {
        let (dir, registry) = installed();
        let first = registry.digest().to_string();
        let mut next = manifest(1, Some(first.clone()));
        next.shared
            .get_mut("kev-0.6b")
            .unwrap()
            .artifact
            .artifact_signature = digest_of('f');
        let registry = Registry::update(dir.path(), next).unwrap();
        assert_eq!(registry.sequence(), 1);

        let earlier = Registry::revision(dir.path(), &first).unwrap();
        assert_eq!(earlier.sequence, 0);
        assert_eq!(
            earlier.shared["kev-0.6b"].artifact.artifact_signature,
            digest_of('a')
        );

        // Rollback is a revision, not a rewrite: sequence 2 restores the
        // old binding and names sequence 1 as what it supersedes.
        let rollback = manifest(2, Some(registry.digest().to_string()));
        let registry = Registry::update(dir.path(), rollback).unwrap();
        let admission = registry.authorize(Some("acme"), "kev-0.6b").unwrap();
        admission.verify(&published(&digest_of('a'))).unwrap();
        let middle = Registry::revision(dir.path(), &earlier_digest(dir.path(), 1)).unwrap();
        assert_eq!(middle.sequence, 1);
    }

    /// The digest a revision line names, for the rollback test.
    fn earlier_digest(dir: &Path, sequence: u64) -> String {
        std::fs::read_to_string(dir.join("revisions.jsonl"))
            .unwrap()
            .lines()
            .map(|line| serde_json::from_str::<Revision>(line).unwrap())
            .find(|revision| revision.sequence == sequence)
            .unwrap()
            .digest
    }

    #[test]
    fn a_stale_update_is_refused() {
        let (dir, registry) = installed();
        let first = registry.digest().to_string();
        let next = manifest(1, Some(first.clone()));
        Registry::update(dir.path(), next).unwrap();
        // A writer that read revision 0 tries to land its update now —
        // the sequence is plausible but the digest it supersedes is not
        // the installed one.
        let stale = manifest(2, Some(first));
        assert!(matches!(
            Registry::update(dir.path(), stale),
            Err(Trouble::Stale { .. })
        ));
        let wrong_sequence = manifest(3, None);
        assert!(matches!(
            Registry::update(dir.path(), wrong_sequence),
            Err(Trouble::Sequence { .. })
        ));
    }

    #[test]
    fn a_tampered_manifest_does_not_open() {
        let (dir, _registry) = installed();
        let path = dir.path().join(CURRENT);
        let text = std::fs::read_to_string(&path).unwrap();
        std::fs::write(&path, text.replacen("acme", "initech", 1)).unwrap();
        assert!(matches!(
            Registry::open(dir.path()),
            Err(Trouble::Invalid(_))
        ));
    }

    #[test]
    fn a_trained_lane_without_a_promotion_reference_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let mut manifest = manifest(0, None);
        manifest.tenants.get_mut("acme").unwrap().doors.insert(
            "acme-trained".to_string(),
            Binding {
                lane: Lane::Trained,
                promotion: None,
                ..dedicated_binding(&digest_of('c'))
            },
        );
        let trouble = Registry::install(dir.path(), manifest).unwrap_err();
        assert!(matches!(trouble, Trouble::Invalid(_)));
    }

    #[test]
    fn a_tenant_binding_cannot_shadow_a_shared_door() {
        let dir = tempfile::tempdir().unwrap();
        let mut manifest = manifest(0, None);
        manifest
            .tenants
            .get_mut("acme")
            .unwrap()
            .doors
            .insert("kev-0.6b".to_string(), dedicated_binding(&digest_of('c')));
        let trouble = Registry::install(dir.path(), manifest).unwrap_err();
        assert!(matches!(trouble, Trouble::Invalid(_)));
    }

    #[test]
    fn an_unpinned_binding_checks_the_name_and_says_so() {
        let dir = tempfile::tempdir().unwrap();
        let mut manifest = manifest(0, None);
        manifest.shared.get_mut("kev-0.6b").unwrap().artifact = Expected {
            model: "jev".to_string(),
            adapter: None,
            artifact_signature: String::new(),
            execution: BTreeMap::new(),
        };
        let registry = Registry::install(dir.path(), manifest).unwrap();
        let admission = registry.authorize(None, "kev-0.6b").unwrap();
        assert!(!admission.pins_artifact());
        let mut card = published("unverifiable");
        card.model = "jev".to_string();
        card.artifact_signature = String::new();
        card.execution.clear();
        admission.verify(&card).unwrap();
        card.model = "kev-4b".to_string();
        assert!(matches!(
            admission.verify(&card),
            Err(Fault::ModelMoved { .. })
        ));
    }
}
