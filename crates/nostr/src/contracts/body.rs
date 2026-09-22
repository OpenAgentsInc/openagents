//! Typed shared bodies: references, locks, effects, evidence, and outcomes.
//!
//! Validators enforce the contract before any effect. A passing check does
//! not grant permission, prove execution, or fetch a locator.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::{Map, Value};

use super::error::{ContractError, RefusalCode};
use super::json::{SAFE_INTEGER, digest_bytes, digest_value, parse_strict};
use crate::domain::Event;

const HEX: &str = "0123456789abcdef";

/// Kind of a private artifact envelope.
pub const ARTIFACT_ENVELOPE_KIND: u16 = 3188;
/// Discovery marker on that envelope.
pub const ARTIFACT_MARKER: &str = "oa:artifact:v1";
/// Media type required of a [`SchemaRef`].
pub const SCHEMA_MEDIA_TYPE: &str = "application/schema+json";

/// An exact `sha256:` digest.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Digest(pub String);

/// A NIP-01 event a reference claims.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EventRef {
    /// Event id.
    pub id: String,
    /// Author.
    pub pubkey: String,
    /// Kind.
    pub kind: u16,
    /// Optional `kind:pubkey:identifier` coordinate. It does not replace `id`.
    pub coordinate: Option<String>,
}

/// A hint that is never fetched by this layer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SourceHint {
    /// A URL the host may ignore.
    Url(String),
    /// An event the host may ignore.
    Event(EventRef),
}

/// Exact-byte identity of an artifact.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArtifactRef {
    /// Digest of the exact bytes.
    pub digest: String,
    /// Exact length of those bytes.
    pub size: u64,
    /// Lowercase media type. It grants no execution authority.
    pub media_type: String,
    /// Schema or dialect identifier for a structured artifact.
    pub schema: Option<String>,
    /// Optional declaring event.
    pub event: Option<EventRef>,
    /// Locator hints. They are not resolved here.
    pub sources: Vec<SourceHint>,
}

/// A qualified component identity and the bytes it names.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DefinitionRef {
    /// `<pubkey>:<package>/<component>`.
    pub id: String,
    /// Bytes of the definition.
    pub artifact: ArtifactRef,
    /// Publisher declaration, when the definition is remote.
    pub event: Option<EventRef>,
}

/// An [`ArtifactRef`] whose media type is a JSON Schema document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SchemaRef(pub ArtifactRef);

/// One locked definition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LockEntry {
    /// Qualified id.
    pub id: String,
    /// Pinned definition.
    pub definition: DefinitionRef,
    /// Qualified ids this entry depends on.
    pub dependencies: Vec<String>,
}

/// A resolved, sorted lock.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Lock {
    /// Canonical digest of the lock body.
    pub digest: String,
    /// Root qualified id.
    pub root: String,
    /// Entries sorted by id.
    pub entries: Vec<LockEntry>,
}

/// Finite ceilings for one resolution. Zero refuses the first unit of work.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResolveLimits {
    /// How many definitions may be walked.
    pub max_definitions: usize,
    /// How deep a dependency chain may be.
    pub max_depth: usize,
    /// How many content bytes may be retained.
    pub max_bytes: usize,
    /// How many store lookups may be attempted.
    pub max_attempts: usize,
}

impl Default for ResolveLimits {
    fn default() -> Self {
        Self {
            max_definitions: 128,
            max_depth: 32,
            max_bytes: 8 * 1024 * 1024,
            max_attempts: 128,
        }
    }
}

/// Bytes a lock retained. Keys are digests. Missing digests are absent,
/// not replaced by another version.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedBytes {
    /// Lock digest that selected these bytes.
    pub lock: String,
    /// Digest to exact bytes.
    pub by_digest: BTreeMap<String, Vec<u8>>,
}

/// Effect classes. Empty lists and false booleans deny that class.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Effects {
    /// Logical read scopes.
    pub reads: Vec<String>,
    /// Logical mutation scopes.
    pub writes: Vec<String>,
    /// Logical network scopes.
    pub network: Vec<String>,
    /// Whether a process may be created.
    pub process: bool,
    /// Whether work may be delegated.
    pub delegates: bool,
    /// Whether spend is requested.
    pub spend: bool,
}

/// A ceiling name from the initial vocabulary.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Bound {
    /// Elapsed deadline, including children.
    WallMs,
    /// Enforceable memory.
    MemoryBytes,
    /// Input bytes.
    InputBytes,
    /// Output bytes.
    OutputBytes,
    /// Read bytes.
    ReadBytes,
    /// Stored bytes.
    StorageBytes,
    /// Invocations.
    Calls,
    /// Attempts.
    Attempts,
    /// Simultaneously active work.
    Concurrency,
    /// Composition depth.
    Depth,
    /// Wasm fuel, only where the host meters it.
    Fuel,
    /// Monetary ceiling. Requires a currency.
    SpendMicrounits,
}

/// Who claims to enforce a bound.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Assurance {
    /// The host enforces the ceiling.
    Host,
    /// An explicitly trusted executor contract claims it.
    TrustedExecutor,
}

/// One assigned ceiling.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoundAssignment {
    /// Which ceiling.
    pub bound: Bound,
    /// The ceiling value.
    pub ceiling: u64,
    /// Optional minimum. It must not exceed `ceiling`.
    pub minimum: Option<u64>,
    /// Host mechanism or trusted-executor claim.
    pub assurance: Assurance,
    /// Host mechanism name. A slug, not a command.
    pub mechanism: String,
    /// Required when `bound` is [`Bound::SpendMicrounits`].
    pub currency: Option<String>,
}

/// A spend hold. Unknown cost is not a zero balance.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reservation {
    /// 32-byte id.
    pub id: String,
    /// Parent hold, when this draw shares one.
    pub parent: Option<String>,
    /// Currency code.
    pub currency: String,
    /// Ceiling, when known.
    pub ceiling_microunits: Option<u64>,
    /// Amount already drawn, when known.
    pub spent_microunits: Option<u64>,
    /// True when part of the cost is unknown.
    pub unknown: bool,
}

/// Where evidence came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceKind {
    /// A repository snapshot.
    Repository,
    /// A command observation.
    Command,
    /// A decision record.
    Decision,
    /// A plugin product.
    Plugin,
    /// A delegated product.
    Delegate,
    /// A document.
    Document,
    /// An admitted external source.
    External,
}

/// How completely a capture kept its source.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Capture {
    /// Whether the capture claims complete source coverage.
    pub complete: bool,
    /// Bytes left out. `None` means the count is unknown.
    pub omitted_bytes: Option<u64>,
}

/// One evidence descriptor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Evidence {
    /// Digest of the descriptor with `id` removed.
    pub id: String,
    /// Content bytes.
    pub content: ArtifactRef,
    /// Source class.
    pub source: SourceKind,
    /// Capture completeness.
    pub capture: Capture,
    /// Parent evidence ids, empty for an original.
    pub derived_from: Vec<String>,
    /// Task id.
    pub task: String,
    /// Pubkeys, or empty for local use only.
    pub recipients: Vec<String>,
    /// Present on a derivative.
    pub transform: Option<DefinitionRef>,
}

/// Whether a context manifest covered its required evidence.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Coverage {
    /// Every required entry is present and none were omitted.
    Complete,
    /// The manifest names a gap.
    Partial,
    /// Coverage was not established.
    Unknown,
}

/// One context entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContextEntry {
    /// Evidence descriptor digest.
    pub evidence: String,
    /// Representation bytes.
    pub representation: ArtifactRef,
    /// Whether the entry is required.
    pub mandatory: bool,
}

/// A context manifest.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContextManifest {
    /// Digest of the canonical manifest.
    pub digest: String,
    /// Pubkey or `local`.
    pub recipient: String,
    /// Policy artifact.
    pub policy: ArtifactRef,
    /// Ordered entries.
    pub entries: Vec<ContextEntry>,
    /// Evidence digests left out.
    pub omissions: Vec<String>,
    /// Coverage claim.
    pub coverage: Coverage,
}

/// Consistency of an external observation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Consistency {
    /// An immutable version under the adapter contract.
    Immutable,
    /// A revision that can precondition a later write.
    Conditional,
    /// A point-in-time read with no snapshot guarantee.
    Observational,
}

/// An external observation. A content hash alone is not a snapshot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Observation {
    /// Host-scoped resource id.
    pub resource_id: String,
    /// Host-scoped resource scope.
    pub resource_scope: String,
    /// Adapter that produced the observation.
    pub adapter: DefinitionRef,
    /// Unix seconds.
    pub captured_at: u64,
    /// Provider revision, when the source has one.
    pub provider_revision: Option<String>,
    /// Retained content, when any.
    pub content: Option<ArtifactRef>,
    /// What the observation guarantees.
    pub consistency: Consistency,
    /// Optional expiry. It must follow `captured_at`.
    pub valid_until: Option<u64>,
}

/// Shared execution outcome. Distinct from verification and integration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    /// The attempt finished.
    Completed,
    /// The attempt was refused before or instead of the effect.
    Refused,
    /// The attempt failed.
    Failed,
    /// The attempt was cancelled.
    Cancelled,
    /// The outcome was not established.
    Unknown,
}

/// Domain acceptance of an observed result.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verification {
    /// The checker passed.
    Passed,
    /// The checker failed.
    Failed,
    /// The checker could not decide.
    Unverifiable,
    /// No checker ran.
    NotRun,
}

/// Whether a proposal was accepted into its destination.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Integration {
    /// Accepted into the authoritative destination.
    Accepted,
    /// Rejected.
    Rejected,
    /// Not yet decided.
    Pending,
    /// This domain has no adoption step.
    NotRequested,
}

/// A usage figure. `None` is unknown, not zero.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Usage {
    /// Elapsed milliseconds, when known.
    pub wall_ms: Option<u64>,
    /// Spend, when known.
    pub spend_microunits: Option<u64>,
}

/// The shared execution receipt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecutionReceipt {
    /// Run id.
    pub run: String,
    /// Step slug.
    pub step: String,
    /// Attempt number, starting at 1.
    pub attempt: u64,
    /// Whether an effect was dispatched.
    pub dispatched: bool,
    /// Component definition.
    pub component: DefinitionRef,
    /// Lock digest.
    pub lock: String,
    /// Input artifact.
    pub input: ArtifactRef,
    /// Context manifest digest.
    pub context: String,
    /// Authority record.
    pub authority: ArtifactRef,
    /// Enforcement plan record.
    pub enforcement: ArtifactRef,
    /// Actual recipient, pubkey or `local`.
    pub recipient: String,
    /// Known usage.
    pub usage: Usage,
    /// Output artifact, when there is one.
    pub output: Option<ArtifactRef>,
    /// Execution outcome.
    pub outcome: Outcome,
    /// Verification outcome.
    pub verification: Verification,
    /// Integration outcome.
    pub integration: Integration,
}

/// Tag routing of a kind-3188 envelope, checked before decryption.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnvelopeRoute {
    /// The single `p` recipient.
    pub recipient: String,
    /// The random mailbox in `h`. Not a guild id.
    pub mailbox: String,
}

/// Decrypted envelope body.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArtifactEnvelope {
    /// The artifact identity.
    pub artifact: ArtifactRef,
    /// Unix seconds the sender observed.
    pub issued_at: u64,
    /// Requested retention instant, later than `issued_at`.
    pub retain_until: u64,
}

/// Whether `reader` may see `evidence`.
///
/// Knowing `evidence.id` does not admit a reader who is outside `recipients`.
/// An empty recipient list admits only the local host.
#[must_use]
pub fn may_read(evidence: &Evidence, reader: &str) -> bool {
    if evidence.recipients.is_empty() {
        return reader == "local";
    }
    evidence.recipients.iter().any(|allowed| allowed == reader)
}

/// Check exact bytes against an artifact reference.
///
/// # Errors
///
/// Returns [`RefusalCode::IdentityMismatch`] when the length or digest differs.
pub fn check_artifact_bytes(artifact: &ArtifactRef, bytes: &[u8]) -> Result<(), ContractError> {
    if bytes.len() as u64 != artifact.size {
        return Err(ContractError::new(
            RefusalCode::IdentityMismatch,
            "artifact size",
        ));
    }
    if digest_bytes(bytes) != artifact.digest {
        return Err(ContractError::new(
            RefusalCode::IdentityMismatch,
            "artifact digest",
        ));
    }
    Ok(())
}

/// Parse an artifact reference.
///
/// # Errors
///
/// Returns a typed refusal when the object is not an artifact reference.
pub fn parse_artifact(value: &Value) -> Result<ArtifactRef, ContractError> {
    let object = map_of(value, "artifact")?;
    reject_unknown(
        object,
        &["digest", "size", "media_type", "schema", "event", "sources"],
        "artifact",
    )?;
    let artifact = ArtifactRef {
        digest: digest_field(require(object, "digest", "artifact")?, "artifact.digest")?,
        size: nonnegative(require(object, "size", "artifact")?, "artifact.size")?,
        media_type: media_type(require(object, "media_type", "artifact")?)?,
        schema: optional_string(object, "schema", "artifact.schema")?,
        event: optional_event(object, "event", "artifact.event")?,
        sources: sources(object)?,
    };
    Ok(artifact)
}

/// Parse a schema reference and require its schema media type.
///
/// # Errors
///
/// Returns [`RefusalCode::Incompatible`] when the media type is not a schema.
pub fn parse_schema_ref(value: &Value) -> Result<SchemaRef, ContractError> {
    let artifact = parse_artifact(value)?;
    if artifact.media_type != SCHEMA_MEDIA_TYPE {
        return Err(ContractError::new(
            RefusalCode::Incompatible,
            "schema media type",
        ));
    }
    Ok(SchemaRef(artifact))
}

/// Parse a definition reference.
///
/// # Errors
///
/// Returns a typed refusal when the object is not a definition reference.
pub fn parse_definition(value: &Value) -> Result<DefinitionRef, ContractError> {
    let object = map_of(value, "definition")?;
    reject_unknown(object, &["id", "artifact", "event"], "definition")?;
    let id = qualified(require(object, "id", "definition")?, "definition.id")?;
    let artifact = parse_artifact(require(object, "artifact", "definition")?)?;
    let event = optional_event(object, "event", "definition.event")?;
    Ok(DefinitionRef {
        id,
        artifact,
        event,
    })
}

/// Confirm a signed event matches `reference` and mentions `digest` in the
/// signed material.
///
/// # Errors
///
/// Returns [`RefusalCode::IdentityMismatch`] when the signature, id, or
/// binding does not match. A coordinate is not accepted in place of the id.
pub fn authenticate_event(
    reference: &EventRef,
    event: &Event,
    digest: &str,
) -> Result<(), ContractError> {
    if event.id != reference.id || event.pubkey != reference.pubkey || event.kind != reference.kind
    {
        return Err(ContractError::new(
            RefusalCode::IdentityMismatch,
            "event reference",
        ));
    }
    event
        .validate_crypto()
        .map_err(|_| ContractError::new(RefusalCode::IdentityMismatch, "event signature"))?;
    if let Some(coordinate) = &reference.coordinate {
        let identifier = coordinate
            .splitn(3, ':')
            .nth(2)
            .ok_or_else(|| malformed("event coordinate"))?;
        let found = event
            .tags
            .iter()
            .any(|tag| tag.name() == Some("d") && tag.value() == Some(identifier));
        if !found {
            return Err(ContractError::new(
                RefusalCode::IdentityMismatch,
                "event coordinate",
            ));
        }
    }
    let bound = event.content.contains(digest)
        || event
            .tags
            .iter()
            .any(|tag| tag.as_slice().iter().any(|part| part == digest));
    if !bound {
        return Err(ContractError::new(
            RefusalCode::IdentityMismatch,
            "event does not bind the artifact",
        ));
    }
    Ok(())
}

/// Parse a lock, require sorted unique entries, and return its JCS digest.
///
/// # Errors
///
/// Returns a typed refusal for cycles, duplicates, an unsorted entry list,
/// or a root that is not an entry.
pub fn parse_lock(value: &Value) -> Result<Lock, ContractError> {
    let object = extensible(value, "openagents.lock.v1", &["root", "entries"], "lock")?;
    let root_def = parse_definition(require(object, "root", "lock")?)?;
    let entries_value = require(object, "entries", "lock")?
        .as_array()
        .ok_or_else(|| malformed("lock.entries"))?;
    let mut entries = Vec::with_capacity(entries_value.len());
    let mut seen = BTreeSet::new();
    for entry in entries_value {
        let entry_object = map_of(entry, "lock.entry")?;
        reject_unknown(
            entry_object,
            &["id", "definition", "dependencies"],
            "lock.entry",
        )?;
        let id = qualified(require(entry_object, "id", "lock.entry")?, "lock.entry.id")?;
        let definition = parse_definition(require(entry_object, "definition", "lock.entry")?)?;
        if definition.id != id {
            return Err(ContractError::new(
                RefusalCode::IdentityMismatch,
                "lock entry id",
            ));
        }
        let dependencies = string_array(
            require(entry_object, "dependencies", "lock.entry")?,
            "lock.entry.dependencies",
        )?;
        if !seen.insert(id.clone()) {
            return Err(ContractError::new(
                RefusalCode::Conflict,
                "duplicate lock id",
            ));
        }
        entries.push(LockEntry {
            id,
            definition,
            dependencies,
        });
    }
    if entries.windows(2).any(|pair| pair[0].id > pair[1].id) {
        return Err(malformed("lock entries are not sorted by id"));
    }
    let root_matches = entries.iter().filter(|entry| entry.id == root_def.id);
    let Some(root_entry) = root_matches.clone().next() else {
        return Err(malformed("lock root must appear exactly once"));
    };
    if root_matches.count() != 1
        || root_entry.definition.artifact.digest != root_def.artifact.digest
    {
        return Err(ContractError::new(
            RefusalCode::IdentityMismatch,
            "lock root",
        ));
    }
    for entry in &entries {
        for dependency in &entry.dependencies {
            if !seen.contains(dependency) {
                return Err(malformed("lock dependency is not an entry"));
            }
        }
    }
    if cyclic(&entries) {
        return Err(ContractError::new(RefusalCode::Incompatible, "lock cycle"));
    }
    Ok(Lock {
        digest: digest_value(value)?,
        root: root_def.id,
        entries,
    })
}

/// Retain the bytes named by `lock`.
///
/// `store` is consulted only by digest. A different digest, including a
/// caller's latest head, is not a substitute for a missing pin.
///
/// # Errors
///
/// Returns [`RefusalCode::ContentUnavailable`] when a pinned digest is
/// absent, [`RefusalCode::IdentityMismatch`] when the bytes differ, and
/// [`RefusalCode::LimitExceeded`] when a ceiling is crossed.
pub fn resolve_lock(
    lock: &Lock,
    store: &BTreeMap<String, Vec<u8>>,
    limits: ResolveLimits,
) -> Result<ResolvedBytes, ContractError> {
    if lock.entries.len() > limits.max_definitions {
        return Err(ContractError::new(
            RefusalCode::LimitExceeded,
            "lock definitions",
        ));
    }
    let mut retained = BTreeMap::new();
    let mut attempts = 0_usize;
    let mut total = 0_usize;
    for entry in &lock.entries {
        attempts += 1;
        if attempts > limits.max_attempts {
            return Err(ContractError::new(
                RefusalCode::LimitExceeded,
                "lock attempts",
            ));
        }
        let digest = &entry.definition.artifact.digest;
        let Some(bytes) = store.get(digest) else {
            return Err(ContractError::new(
                RefusalCode::ContentUnavailable,
                digest.clone(),
            ));
        };
        check_artifact_bytes(&entry.definition.artifact, bytes)?;
        total += bytes.len();
        if total > limits.max_bytes {
            return Err(ContractError::new(RefusalCode::LimitExceeded, "lock bytes"));
        }
        retained.insert(digest.clone(), bytes.clone());
    }
    let depth = dependency_depth(&lock.entries);
    if depth > limits.max_depth {
        return Err(ContractError::new(RefusalCode::LimitExceeded, "lock depth"));
    }
    Ok(ResolvedBytes {
        lock: lock.digest.clone(),
        by_digest: retained,
    })
}

/// Parse an effects object. Scope names are slugs, not paths or wildcards.
///
/// # Errors
///
/// Returns a typed refusal when a scope is not a logical id.
pub fn parse_effects(value: &Value) -> Result<Effects, ContractError> {
    let object = map_of(value, "effects")?;
    reject_unknown(
        object,
        &[
            "reads",
            "writes",
            "network",
            "process",
            "delegates",
            "spend",
        ],
        "effects",
    )?;
    Ok(Effects {
        reads: scopes(require(object, "reads", "effects")?, "effects.reads")?,
        writes: scopes(require(object, "writes", "effects")?, "effects.writes")?,
        network: scopes(require(object, "network", "effects")?, "effects.network")?,
        process: boolean(require(object, "process", "effects")?, "effects.process")?,
        delegates: boolean(
            require(object, "delegates", "effects")?,
            "effects.delegates",
        )?,
        spend: boolean(require(object, "spend", "effects")?, "effects.spend")?,
    })
}

/// Parse one bound assignment.
///
/// # Errors
///
/// Returns [`RefusalCode::Incompatible`] when a minimum exceeds its ceiling,
/// and [`RefusalCode::Malformed`] when spend omits a currency.
pub fn parse_bound(value: &Value) -> Result<BoundAssignment, ContractError> {
    let object = map_of(value, "bound")?;
    reject_unknown(
        object,
        &[
            "bound",
            "ceiling",
            "minimum",
            "assurance",
            "mechanism",
            "currency",
        ],
        "bound",
    )?;
    let bound = bound_name(require(object, "bound", "bound")?)?;
    let ceiling = nonnegative(require(object, "ceiling", "bound")?, "bound.ceiling")?;
    let minimum = match object.get("minimum") {
        None => None,
        Some(value) => Some(nonnegative(value, "bound.minimum")?),
    };
    if minimum.is_some_and(|minimum| minimum > ceiling) {
        return Err(ContractError::new(
            RefusalCode::Incompatible,
            "minimum exceeds ceiling",
        ));
    }
    let assurance = match string(require(object, "assurance", "bound")?, "bound.assurance")? {
        "host" => Assurance::Host,
        "trusted_executor" => Assurance::TrustedExecutor,
        _ => {
            return Err(ContractError::new(
                RefusalCode::UnsupportedFeature,
                "bound.assurance",
            ));
        }
    };
    let mechanism = slug(require(object, "mechanism", "bound")?, "bound.mechanism")?;
    let currency = match object.get("currency") {
        None => None,
        Some(value) => Some(currency_code(value)?),
    };
    if bound == Bound::SpendMicrounits && currency.is_none() {
        return Err(malformed("spend bound requires a currency"));
    }
    Ok(BoundAssignment {
        bound,
        ceiling,
        minimum,
        assurance,
        mechanism,
        currency,
    })
}

/// Require an assignment for every name in `required`.
///
/// # Errors
///
/// Returns [`RefusalCode::CannotEnforce`] when a required bound is absent.
pub fn check_enforcement(
    required: &[Bound],
    plan: &[BoundAssignment],
) -> Result<(), ContractError> {
    for bound in required {
        if !plan.iter().any(|assignment| assignment.bound == *bound) {
            return Err(ContractError::new(
                RefusalCode::CannotEnforce,
                "required bound has no mechanism",
            ));
        }
    }
    Ok(())
}

/// Parse a reservation.
///
/// # Errors
///
/// Returns a typed refusal when the body is not a reservation.
pub fn parse_reservation(value: &Value) -> Result<Reservation, ContractError> {
    let object = extensible(
        value,
        "openagents.reservation.v1",
        &[
            "id",
            "parent",
            "currency",
            "ceiling_microunits",
            "spent_microunits",
            "unknown",
        ],
        "reservation",
    )?;
    let ceiling = optional_nonnegative(object, "ceiling_microunits")?;
    let spent = optional_nonnegative(object, "spent_microunits")?;
    if let (Some(ceiling), Some(spent)) = (ceiling, spent)
        && spent > ceiling
    {
        return Err(ContractError::new(
            RefusalCode::LimitExceeded,
            "reservation spent",
        ));
    }
    Ok(Reservation {
        id: random_id(require(object, "id", "reservation")?, "reservation.id")?,
        parent: optional_random(object, "parent")?,
        currency: currency_code(require(object, "currency", "reservation")?)?,
        ceiling_microunits: ceiling,
        spent_microunits: spent,
        unknown: boolean(
            require(object, "unknown", "reservation")?,
            "reservation.unknown",
        )?,
    })
}

/// Admit a child hold that draws on `parent`.
///
/// An unknown parent cost refuses. It is not treated as zero remaining and
/// not treated as the full ceiling.
///
/// # Errors
///
/// Returns [`RefusalCode::CannotEnforce`] when the parent cost is unknown,
/// and [`RefusalCode::LimitExceeded`] when the child ceiling does not fit.
pub fn admit_child(parent: &Reservation, child: &Reservation) -> Result<(), ContractError> {
    if parent.unknown || parent.ceiling_microunits.is_none() || parent.spent_microunits.is_none() {
        return Err(ContractError::new(
            RefusalCode::CannotEnforce,
            "unknown reservation is not remaining budget",
        ));
    }
    let ceiling = parent.ceiling_microunits.expect("checked");
    let spent = parent.spent_microunits.expect("checked");
    let remaining = ceiling.saturating_sub(spent);
    let child_ceiling = child.ceiling_microunits.ok_or_else(|| {
        ContractError::new(
            RefusalCode::CannotEnforce,
            "child reservation ceiling is unknown",
        )
    })?;
    if child_ceiling > remaining {
        return Err(ContractError::new(
            RefusalCode::LimitExceeded,
            "child reservation exceeds parent",
        ));
    }
    if child.currency != parent.currency {
        return Err(ContractError::new(
            RefusalCode::Incompatible,
            "reservation currency",
        ));
    }
    Ok(())
}

/// Parse evidence and check that `id` is the digest of the body without `id`.
///
/// # Errors
///
/// Returns a typed refusal when the descriptor is inconsistent.
pub fn parse_evidence(value: &Value) -> Result<Evidence, ContractError> {
    let object = extensible(
        value,
        "openagents.evidence.v1",
        &[
            "id",
            "content",
            "source",
            "capture",
            "derived_from",
            "scope",
            "transform",
            "parameters",
        ],
        "evidence",
    )?;
    let id = digest_field(require(object, "id", "evidence")?, "evidence.id")?;
    let mut without_id = value.clone();
    without_id.as_object_mut().expect("object").remove("id");
    if digest_value(&without_id)? != id {
        return Err(ContractError::new(
            RefusalCode::IdentityMismatch,
            "evidence id",
        ));
    }
    let source_object = map_of(require(object, "source", "evidence")?, "evidence.source")?;
    reject_unknown(
        source_object,
        &["kind", "identity", "version", "adapter", "observation"],
        "evidence.source",
    )?;
    let source = source_kind(require(source_object, "kind", "evidence.source")?)?;
    let capture = parse_capture(require(object, "capture", "evidence")?)?;
    let derived_from = digests(
        require(object, "derived_from", "evidence")?,
        "evidence.derived_from",
    )?;
    let scope = map_of(require(object, "scope", "evidence")?, "evidence.scope")?;
    reject_unknown(
        scope,
        &["task", "recipients", "classification"],
        "evidence.scope",
    )?;
    let recipients = pubkeys(require(scope, "recipients", "evidence.scope")?)?;
    let task = random_id(
        require(scope, "task", "evidence.scope")?,
        "evidence.scope.task",
    )?;
    let _classification = slug(
        require(scope, "classification", "evidence.scope")?,
        "evidence.scope.classification",
    )?;
    if source == SourceKind::External {
        parse_definition(require(source_object, "adapter", "evidence.source")?)?;
        parse_artifact(require(source_object, "observation", "evidence.source")?)?;
    }
    let transform = if derived_from.is_empty() {
        if object.contains_key("transform") {
            return Err(malformed("original evidence has a transform"));
        }
        None
    } else {
        Some(parse_definition(require(object, "transform", "evidence")?)?)
    };
    Ok(Evidence {
        id,
        content: parse_artifact(require(object, "content", "evidence")?)?,
        source,
        capture,
        derived_from,
        task,
        recipients,
        transform,
    })
}

/// Refuse a derivative that claims complete coverage of an incomplete input.
///
/// # Errors
///
/// Returns [`RefusalCode::Incompatible`] when a complete derivative cites an
/// incomplete parent.
pub fn check_derivation(records: &[Evidence]) -> Result<(), ContractError> {
    let by_id: BTreeMap<&str, &Evidence> = records
        .iter()
        .map(|record| (record.id.as_str(), record))
        .collect();
    for record in records {
        if !record.capture.complete {
            continue;
        }
        for parent_id in &record.derived_from {
            let Some(parent) = by_id.get(parent_id.as_str()) else {
                return Err(ContractError::new(
                    RefusalCode::ContentUnavailable,
                    "derived evidence",
                ));
            };
            if !parent.capture.complete {
                return Err(ContractError::new(
                    RefusalCode::Incompatible,
                    "incomplete input cannot yield complete coverage",
                ));
            }
        }
    }
    Ok(())
}

/// Parse a context manifest and hash it.
///
/// # Errors
///
/// Returns a typed refusal when mandatory evidence was omitted or a complete
/// manifest still lists omissions.
pub fn parse_context(value: &Value) -> Result<ContextManifest, ContractError> {
    let object = extensible(
        value,
        "openagents.context.v1",
        &[
            "task",
            "recipient",
            "policy",
            "entries",
            "omissions",
            "coverage",
        ],
        "context",
    )?;
    let recipient = recipient(require(object, "recipient", "context")?)?;
    let coverage = coverage(require(object, "coverage", "context")?)?;
    let entries_value = require(object, "entries", "context")?
        .as_array()
        .ok_or_else(|| malformed("context.entries"))?;
    let mut entries = Vec::with_capacity(entries_value.len());
    for entry in entries_value {
        let entry_object = map_of(entry, "context.entry")?;
        reject_unknown(
            entry_object,
            &["evidence", "representation", "mandatory"],
            "context.entry",
        )?;
        entries.push(ContextEntry {
            evidence: digest_field(
                require(entry_object, "evidence", "context.entry")?,
                "context.entry.evidence",
            )?,
            representation: parse_artifact(require(
                entry_object,
                "representation",
                "context.entry",
            )?)?,
            mandatory: boolean(
                require(entry_object, "mandatory", "context.entry")?,
                "context.entry.mandatory",
            )?,
        });
    }
    let omissions = digests(
        require(object, "omissions", "context")?,
        "context.omissions",
    )?;
    if coverage == Coverage::Complete && !omissions.is_empty() {
        return Err(ContractError::new(
            RefusalCode::Incompatible,
            "complete context lists omissions",
        ));
    }
    for omission in &omissions {
        if entries
            .iter()
            .any(|entry| entry.mandatory && entry.evidence == *omission)
        {
            return Err(ContractError::new(
                RefusalCode::Incompatible,
                "mandatory context was omitted",
            ));
        }
    }
    let _task = random_id(require(object, "task", "context")?, "context.task")?;
    Ok(ContextManifest {
        digest: digest_value(value)?,
        recipient,
        policy: parse_artifact(require(object, "policy", "context")?)?,
        entries,
        omissions,
        coverage,
    })
}

/// Parse an observation. Expiry does not prove the resource stayed fresh.
///
/// # Errors
///
/// Returns a typed refusal when neither a revision nor content is present,
/// or when expiry does not follow capture.
pub fn parse_observation(value: &Value) -> Result<Observation, ContractError> {
    let object = extensible(
        value,
        "openagents.observation.v1",
        &[
            "resource",
            "adapter",
            "captured_at",
            "provider_revision",
            "content",
            "consistency",
            "valid_until",
        ],
        "observation",
    )?;
    let resource = map_of(
        require(object, "resource", "observation")?,
        "observation.resource",
    )?;
    reject_unknown(resource, &["scope", "id"], "observation.resource")?;
    let captured_at = nonnegative(
        require(object, "captured_at", "observation")?,
        "observation.captured_at",
    )?;
    let provider_revision = match object.get("provider_revision") {
        None | Some(Value::Null) => None,
        Some(value) => Some(nonempty(value, "observation.provider_revision")?),
    };
    let content = match object.get("content") {
        None | Some(Value::Null) => None,
        Some(value) => Some(parse_artifact(value)?),
    };
    if provider_revision.is_none() && content.is_none() {
        return Err(ContractError::new(
            RefusalCode::ContentUnavailable,
            "observation has neither revision nor content",
        ));
    }
    let valid_until = match object.get("valid_until") {
        None | Some(Value::Null) => None,
        Some(value) => Some(nonnegative(value, "observation.valid_until")?),
    };
    if valid_until.is_some_and(|until| until <= captured_at) {
        return Err(malformed("observation expiry does not follow capture"));
    }
    let consistency = match string(
        require(object, "consistency", "observation")?,
        "observation.consistency",
    )? {
        "immutable" => Consistency::Immutable,
        "conditional" => Consistency::Conditional,
        "observational" => Consistency::Observational,
        _ => {
            return Err(ContractError::new(
                RefusalCode::UnsupportedFeature,
                "observation.consistency",
            ));
        }
    };
    Ok(Observation {
        resource_id: nonempty(
            require(resource, "id", "observation.resource")?,
            "resource.id",
        )?,
        resource_scope: nonempty(
            require(resource, "scope", "observation.resource")?,
            "resource.scope",
        )?,
        adapter: parse_definition(require(object, "adapter", "observation")?)?,
        captured_at,
        provider_revision,
        content,
        consistency,
        valid_until,
    })
}

/// Parse an execution receipt. Outcome, verification, and integration stay
/// distinct fields.
///
/// # Errors
///
/// Returns a typed refusal when a field collapses those results together.
pub fn parse_receipt(value: &Value) -> Result<ExecutionReceipt, ContractError> {
    let object = extensible(
        value,
        "openagents.execution-receipt.v1",
        &[
            "run",
            "step",
            "attempt",
            "dispatched",
            "component",
            "lock",
            "input",
            "context",
            "authority",
            "enforcement",
            "recipient",
            "usage",
            "output",
            "outcome",
            "verification",
            "integration",
        ],
        "receipt",
    )?;
    if object.contains_key("success") {
        return Err(ContractError::new(
            RefusalCode::UnsupportedFeature,
            "success collapses outcome fields",
        ));
    }
    let usage_object = map_of(require(object, "usage", "receipt")?, "receipt.usage")?;
    reject_unknown(
        usage_object,
        &["wall_ms", "spend_microunits"],
        "receipt.usage",
    )?;
    let output = match object.get("output") {
        None | Some(Value::Null) => None,
        Some(value) => Some(parse_artifact(value)?),
    };
    Ok(ExecutionReceipt {
        run: random_id(require(object, "run", "receipt")?, "receipt.run")?,
        step: slug(require(object, "step", "receipt")?, "receipt.step")?,
        attempt: positive(require(object, "attempt", "receipt")?, "receipt.attempt")?,
        dispatched: boolean(
            require(object, "dispatched", "receipt")?,
            "receipt.dispatched",
        )?,
        component: parse_definition(require(object, "component", "receipt")?)?,
        lock: digest_field(require(object, "lock", "receipt")?, "receipt.lock")?,
        input: parse_artifact(require(object, "input", "receipt")?)?,
        context: digest_field(require(object, "context", "receipt")?, "receipt.context")?,
        authority: parse_artifact(require(object, "authority", "receipt")?)?,
        enforcement: parse_artifact(require(object, "enforcement", "receipt")?)?,
        recipient: recipient(require(object, "recipient", "receipt")?)?,
        usage: Usage {
            wall_ms: optional_known(usage_object, "wall_ms")?,
            spend_microunits: optional_known(usage_object, "spend_microunits")?,
        },
        output,
        outcome: outcome(require(object, "outcome", "receipt")?)?,
        verification: verification(require(object, "verification", "receipt")?)?,
        integration: integration(require(object, "integration", "receipt")?)?,
    })
}

/// Check kind-3188 routing tags.
///
/// # Errors
///
/// Returns a typed refusal when the kind, recipient, mailbox, or marker is wrong.
pub fn envelope_route(event: &Event) -> Result<EnvelopeRoute, ContractError> {
    if event.kind != ARTIFACT_ENVELOPE_KIND {
        return Err(ContractError::new(
            RefusalCode::UnsupportedVersion,
            "artifact envelope kind",
        ));
    }
    let mut recipient = None;
    let mut mailbox = None;
    let mut marker = false;
    for tag in &event.tags {
        match tag.name() {
            Some("p") if recipient.is_none() => {
                recipient = Some(pubkey(tag.value().unwrap_or(""), "envelope.p")?);
            }
            Some("h") if mailbox.is_none() => {
                mailbox = Some(random_id_str(tag.value().unwrap_or(""), "envelope.h")?);
            }
            Some("t") if tag.value() == Some(ARTIFACT_MARKER) => {
                if marker {
                    return Err(malformed("duplicate artifact marker"));
                }
                marker = true;
            }
            Some("t") => {}
            _ => {
                return Err(ContractError::new(
                    RefusalCode::UnsupportedFeature,
                    "unexpected envelope tag",
                ));
            }
        }
    }
    if !marker {
        return Err(malformed("missing artifact marker"));
    }
    Ok(EnvelopeRoute {
        recipient: recipient.ok_or_else(|| malformed("envelope recipient"))?,
        mailbox: mailbox.ok_or_else(|| malformed("envelope mailbox"))?,
    })
}

/// Parse a decrypted envelope body.
///
/// When `inline` is an object, its canonical bytes must be the artifact.
///
/// # Errors
///
/// Returns a typed refusal when the body does not match the artifact.
pub fn parse_envelope_body(bytes: &[u8]) -> Result<ArtifactEnvelope, ContractError> {
    let value = parse_strict(bytes)?;
    let object = extensible(
        &value,
        "openagents.artifact-envelope.v1",
        &["artifact", "inline", "issued_at", "retain_until"],
        "envelope",
    )?;
    let artifact = parse_artifact(require(object, "artifact", "envelope")?)?;
    if artifact.schema.is_none() {
        return Err(malformed("envelope artifact schema is required"));
    }
    match require(object, "inline", "envelope")? {
        Value::Null => {}
        Value::Object(_) => {
            let inline = require(object, "inline", "envelope")?;
            let canonical = super::json::jcs(inline)?;
            check_artifact_bytes(&artifact, &canonical)?;
        }
        _ => return Err(malformed("envelope.inline")),
    }
    let issued_at = nonnegative(
        require(object, "issued_at", "envelope")?,
        "envelope.issued_at",
    )?;
    let retain_until = nonnegative(
        require(object, "retain_until", "envelope")?,
        "envelope.retain_until",
    )?;
    if retain_until <= issued_at {
        return Err(malformed("retention does not follow issuance"));
    }
    Ok(ArtifactEnvelope {
        artifact,
        issued_at,
        retain_until,
    })
}

fn cyclic(entries: &[LockEntry]) -> bool {
    let index: BTreeMap<&str, &LockEntry> = entries
        .iter()
        .map(|entry| (entry.id.as_str(), entry))
        .collect();
    let mut visiting = BTreeSet::new();
    let mut visited = BTreeSet::new();
    fn walk(
        id: &str,
        index: &BTreeMap<&str, &LockEntry>,
        visiting: &mut BTreeSet<String>,
        visited: &mut BTreeSet<String>,
    ) -> bool {
        if visited.contains(id) {
            return false;
        }
        if !visiting.insert(id.to_owned()) {
            return true;
        }
        if let Some(entry) = index.get(id) {
            for dependency in &entry.dependencies {
                if walk(dependency, index, visiting, visited) {
                    return true;
                }
            }
        }
        visiting.remove(id);
        visited.insert(id.to_owned());
        false
    }
    entries
        .iter()
        .any(|entry| walk(&entry.id, &index, &mut visiting, &mut visited))
}

fn dependency_depth(entries: &[LockEntry]) -> usize {
    let index: BTreeMap<&str, &LockEntry> = entries
        .iter()
        .map(|entry| (entry.id.as_str(), entry))
        .collect();
    fn depth(id: &str, index: &BTreeMap<&str, &LockEntry>, seen: &mut BTreeSet<String>) -> usize {
        if !seen.insert(id.to_owned()) {
            return 0;
        }
        let Some(entry) = index.get(id) else {
            return 1;
        };
        let below = entry
            .dependencies
            .iter()
            .map(|dependency| depth(dependency, index, seen))
            .max()
            .unwrap_or(0);
        seen.remove(id);
        below + 1
    }
    entries
        .iter()
        .map(|entry| depth(&entry.id, &index, &mut BTreeSet::new()))
        .max()
        .unwrap_or(0)
}

fn extensible<'a>(
    value: &'a Value,
    version: &str,
    fields: &[&str],
    path: &str,
) -> Result<&'a Map<String, Value>, ContractError> {
    let object = map_of(value, path)?;
    let mut allowed = vec!["v", "requires", "meta"];
    allowed.extend_from_slice(fields);
    reject_unknown(object, &allowed, path)?;
    let found = string(require(object, "v", path)?, &format!("{path}.v"))?;
    if found != version {
        return Err(ContractError::new(
            RefusalCode::UnsupportedVersion,
            format!("{path}.v"),
        ));
    }
    let requires = require(object, "requires", path)?
        .as_array()
        .ok_or_else(|| malformed(format!("{path}.requires")))?;
    let mut seen = BTreeSet::new();
    for feature in requires {
        let name = feature
            .as_str()
            .filter(|name| is_slug(name))
            .ok_or_else(|| malformed(format!("{path}.requires")))?;
        if !seen.insert(name) {
            return Err(malformed(format!("{path}.requires")));
        }
    }
    if !seen.is_empty() {
        return Err(ContractError::new(
            RefusalCode::UnsupportedFeature,
            format!("{path}.requires"),
        ));
    }
    if let Some(meta) = object.get("meta") {
        meta.as_object()
            .ok_or_else(|| malformed(format!("{path}.meta")))?;
    }
    Ok(object)
}

fn map_of<'a>(value: &'a Value, path: &str) -> Result<&'a Map<String, Value>, ContractError> {
    value
        .as_object()
        .ok_or_else(|| malformed(format!("{path} object")))
}

fn require<'a>(
    object: &'a Map<String, Value>,
    key: &str,
    path: &str,
) -> Result<&'a Value, ContractError> {
    object
        .get(key)
        .ok_or_else(|| malformed(format!("{path}.{key}")))
}

fn reject_unknown(
    object: &Map<String, Value>,
    allowed: &[&str],
    path: &str,
) -> Result<(), ContractError> {
    for key in object.keys() {
        if !allowed.contains(&key.as_str()) {
            return Err(ContractError::new(
                RefusalCode::UnsupportedFeature,
                format!("{path}.{key}"),
            ));
        }
    }
    Ok(())
}

fn string<'a>(value: &'a Value, path: &str) -> Result<&'a str, ContractError> {
    value.as_str().ok_or_else(|| malformed(path))
}

fn nonempty(value: &Value, path: &str) -> Result<String, ContractError> {
    let text = string(value, path)?;
    if text.is_empty() || text.len() > 256 {
        return Err(malformed(path));
    }
    Ok(text.to_owned())
}

fn boolean(value: &Value, path: &str) -> Result<bool, ContractError> {
    value.as_bool().ok_or_else(|| malformed(path))
}

fn nonnegative(value: &Value, path: &str) -> Result<u64, ContractError> {
    let Some(number) = value.as_u64() else {
        return Err(malformed(path));
    };
    if i128::from(number) > SAFE_INTEGER {
        return Err(malformed(path));
    }
    Ok(number)
}

fn positive(value: &Value, path: &str) -> Result<u64, ContractError> {
    let number = nonnegative(value, path)?;
    if number == 0 {
        return Err(malformed(path));
    }
    Ok(number)
}

fn optional_nonnegative(
    object: &Map<String, Value>,
    key: &str,
) -> Result<Option<u64>, ContractError> {
    match object.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(value) => Ok(Some(nonnegative(value, key)?)),
    }
}

fn optional_known(object: &Map<String, Value>, key: &str) -> Result<Option<u64>, ContractError> {
    match object.get(key) {
        None => Err(malformed(key)),
        Some(Value::Null) => Ok(None),
        Some(value) => Ok(Some(nonnegative(value, key)?)),
    }
}

fn optional_string(
    object: &Map<String, Value>,
    key: &str,
    path: &str,
) -> Result<Option<String>, ContractError> {
    match object.get(key) {
        None => Ok(None),
        Some(value) => Ok(Some(nonempty(value, path)?)),
    }
}

fn digest_field(value: &Value, path: &str) -> Result<String, ContractError> {
    let text = string(value, path)?;
    let Some(hex) = text.strip_prefix("sha256:") else {
        return Err(malformed(path));
    };
    if !is_hex(hex, 32) {
        return Err(malformed(path));
    }
    Ok(text.to_owned())
}

fn random_id(value: &Value, path: &str) -> Result<String, ContractError> {
    random_id_str(string(value, path)?, path)
}

fn random_id_str(value: &str, path: &str) -> Result<String, ContractError> {
    if !is_hex(value, 32) {
        return Err(malformed(path));
    }
    Ok(value.to_owned())
}

fn optional_random(
    object: &Map<String, Value>,
    key: &str,
) -> Result<Option<String>, ContractError> {
    match object.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(value) => Ok(Some(random_id(value, key)?)),
    }
}

fn pubkey(value: &str, path: &str) -> Result<String, ContractError> {
    if !is_hex(value, 32) {
        return Err(malformed(path));
    }
    Ok(value.to_owned())
}

fn is_hex(value: &str, bytes: usize) -> bool {
    value.len() == bytes * 2 && value.bytes().all(|byte| HEX.as_bytes().contains(&byte))
}

fn is_slug(value: &str) -> bool {
    let mut chars = value.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    if value.len() > 64 || !first.is_ascii_lowercase() && !first.is_ascii_digit() {
        return false;
    }
    chars.all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '_' || ch == '-')
}

fn slug(value: &Value, path: &str) -> Result<String, ContractError> {
    let text = string(value, path)?;
    if !is_slug(text) {
        return Err(malformed(path));
    }
    Ok(text.to_owned())
}

fn qualified(value: &Value, path: &str) -> Result<String, ContractError> {
    let text = string(value, path)?;
    let Some((publisher, rest)) = text.split_once(':') else {
        return Err(malformed(path));
    };
    let Some((package, component)) = rest.split_once('/') else {
        return Err(malformed(path));
    };
    if component.contains('/') || !is_hex(publisher, 32) || !is_slug(package) || !is_slug(component)
    {
        return Err(malformed(path));
    }
    Ok(text.to_owned())
}

fn media_type(value: &Value) -> Result<String, ContractError> {
    let text = string(value, "media_type")?;
    let Some((type_name, subtype)) = text.split_once('/') else {
        return Err(malformed("media_type"));
    };
    if type_name.is_empty()
        || subtype.is_empty()
        || text.chars().any(|ch| {
            !ch.is_ascii_lowercase() && !ch.is_ascii_digit() && !matches!(ch, '/' | '+' | '-' | '.')
        })
    {
        return Err(malformed("media_type"));
    }
    Ok(text.to_owned())
}

fn currency_code(value: &Value) -> Result<String, ContractError> {
    let text = string(value, "currency")?;
    if !(1..=16).contains(&text.len())
        || !text
            .chars()
            .all(|ch| ch.is_ascii_uppercase() || ch.is_ascii_digit() || ch == '_')
    {
        return Err(malformed("currency"));
    }
    Ok(text.to_owned())
}

fn scopes(value: &Value, path: &str) -> Result<Vec<String>, ContractError> {
    let array = value.as_array().ok_or_else(|| malformed(path))?;
    array.iter().map(|item| slug(item, path)).collect()
}

fn string_array(value: &Value, path: &str) -> Result<Vec<String>, ContractError> {
    let array = value.as_array().ok_or_else(|| malformed(path))?;
    array.iter().map(|item| qualified(item, path)).collect()
}

fn digests(value: &Value, path: &str) -> Result<Vec<String>, ContractError> {
    let array = value.as_array().ok_or_else(|| malformed(path))?;
    array.iter().map(|item| digest_field(item, path)).collect()
}

fn pubkeys(value: &Value) -> Result<Vec<String>, ContractError> {
    let array = value.as_array().ok_or_else(|| malformed("recipients"))?;
    array
        .iter()
        .map(|item| pubkey(string(item, "recipient")?, "recipient"))
        .collect()
}

fn optional_event(
    object: &Map<String, Value>,
    key: &str,
    path: &str,
) -> Result<Option<EventRef>, ContractError> {
    match object.get(key) {
        None => Ok(None),
        Some(value) => Ok(Some(parse_event_ref(value, path)?)),
    }
}

fn parse_event_ref(value: &Value, path: &str) -> Result<EventRef, ContractError> {
    let object = map_of(value, path)?;
    reject_unknown(object, &["id", "pubkey", "kind", "coordinate"], path)?;
    let kind = nonnegative(require(object, "kind", path)?, &format!("{path}.kind"))?;
    let kind = u16::try_from(kind).map_err(|_| malformed(format!("{path}.kind")))?;
    let id = random_id(require(object, "id", path)?, &format!("{path}.id"))?;
    let pubkey = pubkey(
        string(require(object, "pubkey", path)?, &format!("{path}.pubkey"))?,
        &format!("{path}.pubkey"),
    )?;
    let coordinate = match object.get("coordinate") {
        None => None,
        Some(value) => {
            let text = nonempty(value, &format!("{path}.coordinate"))?;
            let mut parts = text.splitn(3, ':');
            let kind_text = parts.next().unwrap_or("");
            let pubkey_text = parts.next().unwrap_or("");
            let identifier = parts.next().unwrap_or("");
            if kind_text.parse::<u16>().ok().as_ref() != Some(&kind)
                || pubkey_text != pubkey
                || identifier.is_empty()
            {
                return Err(malformed(format!("{path}.coordinate")));
            }
            Some(text)
        }
    };
    Ok(EventRef {
        id,
        pubkey,
        kind,
        coordinate,
    })
}

fn sources(object: &Map<String, Value>) -> Result<Vec<SourceHint>, ContractError> {
    let Some(value) = object.get("sources") else {
        return Ok(Vec::new());
    };
    let array = value
        .as_array()
        .ok_or_else(|| malformed("artifact.sources"))?;
    let mut hints = Vec::with_capacity(array.len());
    for hint in array {
        let hint_object = map_of(hint, "artifact.source")?;
        reject_unknown(hint_object, &["url", "event"], "artifact.source")?;
        match (hint_object.get("url"), hint_object.get("event")) {
            (Some(url), None) => hints.push(SourceHint::Url(nonempty(url, "artifact.source.url")?)),
            (None, Some(event)) => {
                hints.push(SourceHint::Event(parse_event_ref(
                    event,
                    "artifact.source.event",
                )?));
            }
            _ => return Err(malformed("artifact.source")),
        }
    }
    Ok(hints)
}

fn parse_capture(value: &Value) -> Result<Capture, ContractError> {
    let object = map_of(value, "evidence.capture")?;
    reject_unknown(
        object,
        &["complete", "omitted_bytes", "reason"],
        "evidence.capture",
    )?;
    let omitted_bytes = match require(object, "omitted_bytes", "evidence.capture")? {
        Value::Null => None,
        other => Some(nonnegative(other, "evidence.capture.omitted_bytes")?),
    };
    if let Some(reason) = object.get("reason")
        && !reason.is_null()
    {
        let _reason = nonempty(reason, "evidence.capture.reason")?;
    }
    Ok(Capture {
        complete: boolean(
            require(object, "complete", "evidence.capture")?,
            "evidence.capture.complete",
        )?,
        omitted_bytes,
    })
}

fn source_kind(value: &Value) -> Result<SourceKind, ContractError> {
    Ok(match string(value, "evidence.source.kind")? {
        "repository" => SourceKind::Repository,
        "command" => SourceKind::Command,
        "decision" => SourceKind::Decision,
        "plugin" => SourceKind::Plugin,
        "delegate" => SourceKind::Delegate,
        "document" => SourceKind::Document,
        "external" => SourceKind::External,
        _ => {
            return Err(ContractError::new(
                RefusalCode::UnsupportedFeature,
                "evidence.source.kind",
            ));
        }
    })
}

fn bound_name(value: &Value) -> Result<Bound, ContractError> {
    Ok(match string(value, "bound.bound")? {
        "wall_ms" => Bound::WallMs,
        "memory_bytes" => Bound::MemoryBytes,
        "input_bytes" => Bound::InputBytes,
        "output_bytes" => Bound::OutputBytes,
        "read_bytes" => Bound::ReadBytes,
        "storage_bytes" => Bound::StorageBytes,
        "calls" => Bound::Calls,
        "attempts" => Bound::Attempts,
        "concurrency" => Bound::Concurrency,
        "depth" => Bound::Depth,
        "fuel" => Bound::Fuel,
        "spend_microunits" => Bound::SpendMicrounits,
        _ => {
            return Err(ContractError::new(
                RefusalCode::UnsupportedFeature,
                "bound.bound",
            ));
        }
    })
}

fn coverage(value: &Value) -> Result<Coverage, ContractError> {
    Ok(match string(value, "context.coverage")? {
        "complete" => Coverage::Complete,
        "partial" => Coverage::Partial,
        "unknown" => Coverage::Unknown,
        _ => {
            return Err(ContractError::new(
                RefusalCode::UnsupportedFeature,
                "context.coverage",
            ));
        }
    })
}

fn recipient(value: &Value) -> Result<String, ContractError> {
    let text = string(value, "recipient")?;
    if text == "local" {
        return Ok(text.to_owned());
    }
    pubkey(text, "recipient")
}

fn outcome(value: &Value) -> Result<Outcome, ContractError> {
    Ok(match string(value, "receipt.outcome")? {
        "completed" => Outcome::Completed,
        "refused" => Outcome::Refused,
        "failed" => Outcome::Failed,
        "cancelled" => Outcome::Cancelled,
        "unknown" => Outcome::Unknown,
        _ => {
            return Err(ContractError::new(
                RefusalCode::UnsupportedFeature,
                "receipt.outcome",
            ));
        }
    })
}

fn verification(value: &Value) -> Result<Verification, ContractError> {
    Ok(match string(value, "receipt.verification")? {
        "passed" => Verification::Passed,
        "failed" => Verification::Failed,
        "unverifiable" => Verification::Unverifiable,
        "not_run" => Verification::NotRun,
        _ => {
            return Err(ContractError::new(
                RefusalCode::UnsupportedFeature,
                "receipt.verification",
            ));
        }
    })
}

fn integration(value: &Value) -> Result<Integration, ContractError> {
    Ok(match string(value, "receipt.integration")? {
        "accepted" => Integration::Accepted,
        "rejected" => Integration::Rejected,
        "pending" => Integration::Pending,
        "not_requested" => Integration::NotRequested,
        _ => {
            return Err(ContractError::new(
                RefusalCode::UnsupportedFeature,
                "receipt.integration",
            ));
        }
    })
}

fn malformed(detail: impl Into<String>) -> ContractError {
    ContractError::new(RefusalCode::Malformed, detail)
}
