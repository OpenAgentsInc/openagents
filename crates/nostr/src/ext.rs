//! NIP-EXT v1 release, listing, revocation, and migration records.
//!
//! A release event pins a manifest. A version label is not that pin.
//! Publication, installation, enablement, grants, and admission stay
//! separate, and none of them runs a script, a probe, or inference.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::{Map, Value};

use crate::contracts::{
    ArtifactRef, ContractError, RefusalCode, check_artifact_bytes, parse_artifact,
    parse_schema_ref, parse_strict,
};
use crate::domain::{Event, Tag};

/// Addressable package listing.
pub const LISTING_KIND: u16 = 30_184;
/// Immutable release.
pub const RELEASE_KIND: u16 = 3_184;
/// Irreversible revocation of one release.
pub const REVOCATION_KIND: u16 = 3_185;
/// Two-party namespace migration.
pub const MIGRATION_KIND: u16 = 3_186;
/// Addressable revocation checkpoint.
pub const CHECKPOINT_KIND: u16 = 30_185;
/// NIP-94 file locator. It is not an execution pin.
pub const LOCATOR_KIND: u16 = 1_063;
/// NIP-51 curation set. It is not an execution pin.
pub const CURATION_KIND: u16 = 30_001;

const COMPONENT_KINDS: &[&str] = &[
    "program",
    "plugin",
    "capability",
    "decision-function",
    "skill",
    "source",
    "checker",
    "operation",
    "ai-signature",
    "ai-implementation",
    "guidance",
    "schema",
];

/// Who asked. Both surfaces use [`authorize`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Surface {
    /// An operator at a terminal.
    Human,
    /// A model-facing authoring call.
    Model,
}

/// A catalog phase. Each one is a different effect.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    /// Publish a listing or a release.
    Publish,
    /// Commit a verified lock.
    Install,
    /// Allow activation.
    Enable,
    /// Record a grant.
    Grant,
    /// Admit a dispatch.
    Admit,
}

/// A side effect a phase must not smuggle in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    /// No extra effect.
    None,
    /// An install script or build.
    Script,
    /// A probe.
    Probe,
    /// An inference call.
    Infer,
}

/// One authorization request. The surface does not change the answer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Authorization {
    /// Human or model.
    pub surface: Surface,
    /// The phase being asked for.
    pub phase: Phase,
    /// A side effect, if the caller also asked for one.
    pub side: Side,
    /// Whether the operator authorized this phase.
    pub authorized: bool,
}

/// Where an installation stands.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Install {
    /// Nothing is installed.
    Absent,
    /// A verified lock is staged and not yet committed.
    Staged {
        /// The lock waiting to commit.
        lock: String,
        /// The lock a commit would replace, when this is an update.
        previous: Option<String>,
    },
    /// The committed lock.
    Installed {
        /// Lock digest or event id.
        lock: String,
    },
    /// Uninstall has removed the activation reference.
    Disabled {
        /// Lock whose bytes may still be on disk.
        lock: String,
    },
    /// Cleanup failed. The package stays inactive.
    Tombstone {
        /// Lock whose bytes still need cleanup.
        lock: String,
    },
}

/// One install transition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InstallStep {
    /// Put a verified lock in staging.
    Stage {
        /// Lock identity.
        lock: String,
        /// Whether closure verification already succeeded.
        verified: bool,
    },
    /// Atomically commit the staged lock.
    Commit,
    /// Select a previously verified lock.
    Rollback {
        /// The earlier lock.
        previous: String,
        /// Whether that lock is still eligible.
        eligible: bool,
    },
    /// Stop new activation and drop the installation reference.
    BeginUninstall,
    /// Remove unreferenced bytes.
    Cleanup {
        /// Whether the bytes were removed.
        succeeded: bool,
    },
    /// Try to make a tombstone active again.
    Reactivate,
}

/// Verified revocations retained for one package.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RevocationKnowledge {
    /// Highest accepted checkpoint revision.
    pub revision: u64,
    /// Union of revocation event ids.
    pub revocations: BTreeSet<String>,
}

/// A parsed package manifest.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Manifest {
    /// `<pubkey>:<slug>`.
    pub package: String,
    /// Human-readable version label. It is not the pin.
    pub version: String,
    /// Normalized file paths.
    pub files: Vec<FileEntry>,
    /// Exact dependency release event ids.
    pub dependencies: Vec<String>,
    /// Component definition digests that must be listed.
    pub definitions: Vec<String>,
}

/// One file the manifest lists.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileEntry {
    /// Normalized relative path.
    pub path: String,
    /// `sha256:` digest.
    pub digest: String,
    /// Exact byte length.
    pub size: u64,
    /// Media type.
    pub media_type: String,
}

/// Bytes and paths the closure check reads.
pub struct Closure<'a> {
    /// Manifest under check.
    pub manifest: &'a Manifest,
    /// Digest to exact bytes.
    pub files: &'a BTreeMap<String, Vec<u8>>,
    /// Paths present in the staging directory.
    pub staged: &'a [String],
    /// Release id to its manifest, including the root.
    pub dependencies: &'a BTreeMap<String, Manifest>,
    /// Release id of `manifest`.
    pub root: &'a str,
    /// Maximum total listed size, checked before a copy.
    pub byte_limit: u64,
}

/// A checkpoint's freshness inputs.
#[derive(Debug, Clone, Copy)]
pub struct Freshness {
    /// Checkpoint `as_of`.
    pub as_of: u64,
    /// Checkpoint `valid_until`.
    pub valid_until: u64,
    /// Trusted clock.
    pub now: u64,
    /// Allowed future skew.
    pub skew: u64,
    /// Whether a checkpoint event was returned.
    pub present: bool,
    /// Whether the answer listed no revocations and was not a signed checkpoint.
    pub empty_answer: bool,
    /// Strict policy requires a fresh checkpoint.
    pub strict: bool,
    /// The caller named an explicit offline pin.
    pub explicit_pin: bool,
}

/// Authorize one phase. Human and model callers share this function.
///
/// # Errors
///
/// Returns [`RefusalCode::NotAdmitted`] without authorization, and
/// [`RefusalCode::CannotEnforce`] when the phase would run a script, a
/// probe, or inference.
pub fn authorize(request: &Authorization) -> Result<(), ContractError> {
    let _ = request.surface;
    if !request.authorized {
        return Err(ContractError::new(RefusalCode::NotAdmitted, "publish"));
    }
    match request.side {
        Side::Script | Side::Probe | Side::Infer => Err(ContractError::new(
            RefusalCode::CannotEnforce,
            "install side effect",
        )),
        Side::None => Ok(()),
    }
}

/// Apply one install transition.
///
/// # Errors
///
/// Returns a refusal when the step is not legal for `state`.
pub fn transition(state: &Install, step: InstallStep) -> Result<Install, ContractError> {
    match (state, step) {
        (
            Install::Absent | Install::Installed { .. },
            InstallStep::Stage {
                lock,
                verified: true,
            },
        ) => {
            let previous = match state {
                Install::Installed { lock } => Some(lock.clone()),
                _ => None,
            };
            Ok(Install::Staged { lock, previous })
        }
        (
            _,
            InstallStep::Stage {
                verified: false, ..
            },
        ) => Err(ContractError::new(
            RefusalCode::Malformed,
            "unverified stage",
        )),
        (Install::Staged { lock, .. }, InstallStep::Commit) => {
            Ok(Install::Installed { lock: lock.clone() })
        }
        (
            Install::Installed { .. },
            InstallStep::Rollback {
                previous,
                eligible: true,
            },
        ) => Ok(Install::Installed { lock: previous }),
        (
            Install::Installed { .. },
            InstallStep::Rollback {
                eligible: false, ..
            },
        ) => Err(ContractError::new(RefusalCode::Stale, "rollback")),
        (Install::Installed { lock }, InstallStep::BeginUninstall) => {
            Ok(Install::Disabled { lock: lock.clone() })
        }
        (Install::Disabled { .. }, InstallStep::Cleanup { succeeded: true }) => Ok(Install::Absent),
        (Install::Disabled { lock }, InstallStep::Cleanup { succeeded: false }) => {
            Ok(Install::Tombstone { lock: lock.clone() })
        }
        (Install::Tombstone { .. }, InstallStep::Cleanup { succeeded: true }) => {
            Ok(Install::Absent)
        }
        (Install::Tombstone { .. }, InstallStep::Reactivate | InstallStep::Commit) => {
            Err(ContractError::new(RefusalCode::Conflict, "tombstone"))
        }
        _ => Err(ContractError::new(
            RefusalCode::Malformed,
            "install transition",
        )),
    }
}

/// The pin an active run keeps. `proposed` is not swapped in.
#[must_use]
pub fn preserve_active_pin<'a>(active: &'a str, proposed: &'a str) -> &'a str {
    let _ = proposed;
    active
}

/// Whether `kind` is an immutable execution pin.
///
/// A listing and a NIP-51 set are replaceable discovery records.
#[must_use]
pub fn execution_pin(kind: u16) -> bool {
    kind == RELEASE_KIND
}

/// Refuse archive expansion. This transport addresses files individually.
///
/// # Errors
///
/// Returns [`RefusalCode::UnsupportedFeature`] for an archive media type.
pub fn expand_archive(media_type: &str) -> Result<(), ContractError> {
    if media_type == "application/zip" || media_type == "application/gzip" {
        return Err(ContractError::new(
            RefusalCode::UnsupportedFeature,
            "archive expansion",
        ));
    }
    Ok(())
}

/// Two releases that share a version label and differ in manifest digest.
///
/// # Errors
///
/// Returns [`RefusalCode::Conflict`] when the digests differ.
pub fn equivocation(left_manifest: &str, right_manifest: &str) -> Result<(), ContractError> {
    if left_manifest == right_manifest {
        Ok(())
    } else {
        Err(ContractError::new(RefusalCode::Conflict, "version label"))
    }
}

/// Bytes for a pinned digest. A different digest in `store` is not a substitute.
///
/// # Errors
///
/// Returns [`RefusalCode::ContentUnavailable`] when `pinned` is absent.
pub fn historical_bytes<'a>(
    pinned: &str,
    store: &'a BTreeMap<String, Vec<u8>>,
) -> Result<&'a [u8], ContractError> {
    store
        .get(pinned)
        .map(Vec::as_slice)
        .ok_or_else(|| ContractError::new(RefusalCode::ContentUnavailable, pinned))
}

/// Fold a checkpoint's revocation list into the retained union.
///
/// # Errors
///
/// Returns [`RefusalCode::Conflict`] for a rewritten revision or a missing
/// revocation, and [`RefusalCode::Stale`] when `revision` would roll backward.
pub fn absorb(
    current: &RevocationKnowledge,
    revision: u64,
    revocations: &[String],
) -> Result<RevocationKnowledge, ContractError> {
    let mut incoming = BTreeSet::new();
    let mut previous = String::new();
    for id in revocations {
        if !incoming.insert(id.clone()) || (!previous.is_empty() && id.as_str() < previous.as_str())
        {
            return Err(ContractError::new(RefusalCode::Malformed, "revocations"));
        }
        previous = id.clone();
    }
    if revision < current.revision {
        return Err(ContractError::new(
            RefusalCode::Stale,
            "checkpoint revision",
        ));
    }
    if revision == current.revision && incoming != current.revocations {
        return Err(ContractError::new(RefusalCode::Conflict, "checkpoint"));
    }
    if !current.revocations.is_subset(&incoming) {
        return Err(ContractError::new(
            RefusalCode::Conflict,
            "omitted revocation",
        ));
    }
    Ok(RevocationKnowledge {
        revision,
        revocations: incoming,
    })
}

/// Decide whether a checkpoint is fresh enough to admit new work.
///
/// # Errors
///
/// Returns [`RefusalCode::Stale`] for a future, expired, missing, or withheld
/// checkpoint under strict policy. An empty answer is not evidence.
pub fn fresh(view: &Freshness) -> Result<(), ContractError> {
    if view.valid_until <= view.as_of {
        return Err(ContractError::new(RefusalCode::Malformed, "valid_until"));
    }
    if view.as_of > view.now.saturating_add(view.skew) {
        return Err(ContractError::new(RefusalCode::Stale, "as_of"));
    }
    if view.now > view.valid_until {
        return Err(ContractError::new(RefusalCode::Stale, "valid_until"));
    }
    if view.empty_answer {
        return Err(ContractError::new(
            RefusalCode::ContentUnavailable,
            "empty revocation query",
        ));
    }
    if !view.present {
        if view.strict && !view.explicit_pin {
            return Err(ContractError::new(RefusalCode::Stale, "missing checkpoint"));
        }
        return Ok(());
    }
    Ok(())
}

/// A NIP-94 locator agrees with the artifact, or it is not that artifact.
///
/// # Errors
///
/// Returns [`RefusalCode::IdentityMismatch`] when MIME type, size, or hash differ.
pub fn locator_matches(event: &Event, artifact: &ArtifactRef) -> Result<(), ContractError> {
    if event.kind != LOCATOR_KIND {
        return Err(ContractError::new(RefusalCode::Malformed, "locator kind"));
    }
    event
        .validate_crypto()
        .map_err(|_| ContractError::new(RefusalCode::IdentityMismatch, "locator signature"))?;
    let mime = one_tag(event, "m")?;
    let hash = one_tag(event, "x")?;
    let size: u64 = one_tag(event, "size")?
        .parse()
        .map_err(|_| malformed("locator size"))?;
    let expected = artifact
        .digest
        .strip_prefix("sha256:")
        .ok_or_else(|| malformed("digest"))?;
    if mime != artifact.media_type || size != artifact.size || hash != expected {
        return Err(ContractError::new(RefusalCode::IdentityMismatch, "locator"));
    }
    Ok(())
}

/// Parse a signed catalog record and require the body to agree with its tags.
///
/// # Errors
///
/// Returns a typed refusal for a bad signature, a tag disagreement, or a
/// body this version does not implement.
pub fn parse_record(event: &Event) -> Result<Value, ContractError> {
    event
        .validate_crypto()
        .map_err(|_| ContractError::new(RefusalCode::IdentityMismatch, "event signature"))?;
    let value = parse_strict(event.content.as_bytes())?;
    let object = as_map(&value, "record")?;
    let record_type = text(require(object, "type", "record")?, "type")?;
    let marker = format!("oa:ext:{record_type}:v1");
    if one_marker(event)? != marker {
        return Err(ContractError::new(
            RefusalCode::IdentityMismatch,
            "extension tag",
        ));
    }
    let kind = match record_type {
        "listing" => LISTING_KIND,
        "release" => RELEASE_KIND,
        "revocation" => REVOCATION_KIND,
        "migration" => MIGRATION_KIND,
        "checkpoint" => CHECKPOINT_KIND,
        _ => {
            return Err(ContractError::new(
                RefusalCode::UnsupportedFeature,
                "extension type",
            ));
        }
    };
    if event.kind != kind {
        return Err(ContractError::new(
            RefusalCode::IdentityMismatch,
            "extension kind",
        ));
    }
    version(object)?;
    match record_type {
        "listing" => parse_listing(event, object)?,
        "release" => parse_release(event, object)?,
        "revocation" => parse_revocation(event, object)?,
        "migration" => parse_migration(event, object)?,
        "checkpoint" => parse_checkpoint(event, object)?,
        _ => {}
    }
    Ok(value)
}

/// Parse a package manifest.
///
/// # Errors
///
/// Returns a typed refusal for an unknown field, a colliding path, or an
/// unsupported component kind.
pub fn parse_manifest(value: &Value) -> Result<Manifest, ContractError> {
    let object = as_map(value, "manifest")?;
    reject(
        object,
        &[
            "v",
            "requires",
            "package",
            "version",
            "license",
            "provenance",
            "components",
            "files",
            "dependencies",
            "meta",
        ],
        "manifest",
    )?;
    if text(require(object, "v", "manifest")?, "v")? != "openagents.package.v1" {
        return Err(ContractError::new(
            RefusalCode::UnsupportedVersion,
            "manifest.v",
        ));
    }
    require_empty(object, "requires")?;
    let package = package_id(require(object, "package", "manifest")?)?;
    let version = label(require(object, "version", "manifest")?)?;
    let _license = text(require(object, "license", "manifest")?, "license")?;
    parse_provenance(require(object, "provenance", "manifest")?)?;
    let components = require(object, "components", "manifest")?
        .as_array()
        .ok_or_else(|| malformed("components"))?;
    let mut definitions = Vec::new();
    let mut seen_components = BTreeSet::new();
    for component in components {
        definitions.push(parse_component(component, &mut seen_components)?);
    }
    let files = parse_files(require(object, "files", "manifest")?)?;
    let dependencies = string_list(require(object, "dependencies", "manifest")?, "dependencies")?;
    Ok(Manifest {
        package,
        version,
        files,
        dependencies,
        definitions,
    })
}

/// Check the manifest against the bytes and dependency manifests supplied.
///
/// # Errors
///
/// Returns a refusal for a missing pin, a cycle, an unlisted path, or a
/// size past `byte_limit`.
pub fn verify_closure(check: &Closure<'_>) -> Result<(), ContractError> {
    let mut total = 0_u64;
    for file in &check.manifest.files {
        let Some(bytes) = check.files.get(&file.digest) else {
            return Err(ContractError::new(
                RefusalCode::ContentUnavailable,
                file.digest.clone(),
            ));
        };
        let artifact = ArtifactRef {
            digest: file.digest.clone(),
            size: file.size,
            media_type: file.media_type.clone(),
            schema: None,
            event: None,
            sources: Vec::new(),
        };
        check_artifact_bytes(&artifact, bytes)?;
        total = total.saturating_add(file.size);
    }
    if total > check.byte_limit {
        return Err(ContractError::new(
            RefusalCode::LimitExceeded,
            "package bytes",
        ));
    }
    let listed: BTreeSet<&str> = check
        .manifest
        .files
        .iter()
        .map(|file| file.path.as_str())
        .collect();
    for path in check.staged {
        let normal = normalize_path(path)?;
        if !listed.contains(normal.as_str()) {
            return Err(ContractError::new(
                RefusalCode::Incompatible,
                "unlisted content",
            ));
        }
    }
    let known: BTreeSet<&str> = check
        .manifest
        .files
        .iter()
        .map(|file| file.digest.as_str())
        .chain(
            check
                .dependencies
                .values()
                .flat_map(|manifest| manifest.files.iter().map(|file| file.digest.as_str())),
        )
        .collect();
    for digest in &check.manifest.definitions {
        if !known.contains(digest.as_str()) {
            return Err(ContractError::new(
                RefusalCode::ContentUnavailable,
                "component bytes",
            ));
        }
    }
    walk_dependencies(check.root, check.dependencies, &mut Vec::new())?;
    Ok(())
}

/// Parse an operation descriptor and require it to name `definition_id`.
///
/// # Errors
///
/// Returns a typed refusal when the descriptor disagrees with the definition.
pub fn parse_operation(value: &Value, definition_id: &str) -> Result<(), ContractError> {
    let object = as_map(value, "operation")?;
    reject(
        object,
        &[
            "v",
            "requires",
            "id",
            "kind",
            "definition",
            "summary",
            "input",
            "output",
            "preconditions",
            "effects",
            "minimum",
            "guidance",
            "evaluation",
            "meta",
        ],
        "operation",
    )?;
    if text(require(object, "v", "operation")?, "v")? != "openagents.operation.v1" {
        return Err(ContractError::new(
            RefusalCode::UnsupportedVersion,
            "operation.v",
        ));
    }
    require_empty(object, "requires")?;
    let id = qualified(require(object, "id", "operation")?)?;
    if id != definition_id {
        return Err(ContractError::new(
            RefusalCode::IdentityMismatch,
            "operation id",
        ));
    }
    if text(require(object, "kind", "operation")?, "kind")? != "operation" {
        return Err(ContractError::new(
            RefusalCode::UnsupportedFeature,
            "operation kind",
        ));
    }
    parse_artifact(require(object, "definition", "operation")?)?;
    let _summary = text(require(object, "summary", "operation")?, "summary")?;
    parse_schema_ref(require(object, "input", "operation")?)?;
    parse_schema_ref(require(object, "output", "operation")?)?;
    let _preconditions = as_map(
        require(object, "preconditions", "operation")?,
        "preconditions",
    )?;
    let _effects = as_map(require(object, "effects", "operation")?, "effects")?;
    let _minimum = as_map(require(object, "minimum", "operation")?, "minimum")?;
    let _guidance = require(object, "guidance", "operation")?
        .as_array()
        .ok_or_else(|| malformed("guidance"))?;
    Ok(())
}

/// Parse a decision-function definition. It cannot name itself as its builder.
///
/// # Errors
///
/// Returns [`RefusalCode::Incompatible`] when `state_builder` is this function.
pub fn parse_function(value: &Value) -> Result<String, ContractError> {
    let object = as_map(value, "function")?;
    reject(
        object,
        &[
            "v",
            "requires",
            "id",
            "input",
            "output",
            "state_builder",
            "questions",
            "policy",
            "model_requirements",
            "bounds",
            "evaluation",
        ],
        "function",
    )?;
    if text(require(object, "v", "function")?, "v")? != "openagents.function.v1" {
        return Err(ContractError::new(
            RefusalCode::UnsupportedVersion,
            "function.v",
        ));
    }
    require_empty(object, "requires")?;
    let id = qualified(require(object, "id", "function")?)?;
    parse_schema_ref(require(object, "input", "function")?)?;
    parse_schema_ref(require(object, "output", "function")?)?;
    let builder = qualified(require(object, "state_builder", "function")?)?;
    if builder == id {
        return Err(ContractError::new(
            RefusalCode::Incompatible,
            "function calls itself",
        ));
    }
    parse_artifact(require(object, "questions", "function")?)?;
    let _policy = qualified(require(object, "policy", "function")?)?;
    let requirements = as_map(
        require(object, "model_requirements", "function")?,
        "model_requirements",
    )?;
    let _apis = require(requirements, "apis", "model_requirements")?
        .as_array()
        .ok_or_else(|| malformed("apis"))?;
    let _schemas = require(requirements, "schemas", "model_requirements")?
        .as_array()
        .ok_or_else(|| malformed("schemas"))?;
    if require(
        requirements,
        "requires_scorable_answer",
        "model_requirements",
    )?
    .as_bool()
    .is_none()
    {
        return Err(malformed("requires_scorable_answer"));
    }
    let _bounds = as_map(require(object, "bounds", "function")?, "bounds")?;
    let _evaluation = require(object, "evaluation", "function")?
        .as_array()
        .ok_or_else(|| malformed("evaluation"))?;
    Ok(id)
}

/// Parse a skill definition. Unknown hook events and shell fields refuse.
///
/// # Errors
///
/// Returns a typed refusal for an unknown event or an unsupported field.
pub fn parse_skill(value: &Value) -> Result<(), ContractError> {
    let object = as_map(value, "skill")?;
    reject(
        object,
        &[
            "v",
            "requires",
            "id",
            "description",
            "body",
            "applicability",
            "allowed_operations",
            "lifetime",
            "hooks",
        ],
        "skill",
    )?;
    if text(require(object, "v", "skill")?, "v")? != "openagents.skill.v1" {
        return Err(ContractError::new(
            RefusalCode::UnsupportedVersion,
            "skill.v",
        ));
    }
    require_empty(object, "requires")?;
    let _id = qualified(require(object, "id", "skill")?)?;
    let _description = text(require(object, "description", "skill")?, "description")?;
    parse_artifact(require(object, "body", "skill")?)?;
    let _applicability = require(object, "applicability", "skill")?
        .as_array()
        .ok_or_else(|| malformed("applicability"))?;
    let _allowed = require(object, "allowed_operations", "skill")?
        .as_array()
        .ok_or_else(|| malformed("allowed_operations"))?;
    let lifetime = text(require(object, "lifetime", "skill")?, "lifetime")?;
    if !matches!(lifetime, "operation" | "task" | "session") {
        return Err(ContractError::new(
            RefusalCode::UnsupportedFeature,
            "skill lifetime",
        ));
    }
    let hooks = require(object, "hooks", "skill")?
        .as_array()
        .ok_or_else(|| malformed("hooks"))?;
    for hook in hooks {
        parse_hook(hook)?;
    }
    Ok(())
}

/// Pair an offer with its acceptance. Grants are not a migration field.
///
/// # Errors
///
/// Returns a refusal when the signers, ids, or package endpoints disagree.
pub fn pair_migration(offer: &Event, accept: &Event) -> Result<(), ContractError> {
    let offer_body = parse_record(offer)?;
    let accept_body = parse_record(accept)?;
    let offer_map = as_map(&offer_body, "offer")?;
    let accept_map = as_map(&accept_body, "accept")?;
    if text(require(offer_map, "role", "offer")?, "role")? != "offer" {
        return Err(ContractError::new(RefusalCode::Malformed, "offer role"));
    }
    if text(require(accept_map, "role", "accept")?, "role")? != "accept" {
        return Err(ContractError::new(RefusalCode::Malformed, "accept role"));
    }
    let from = pubkey_text(require(offer_map, "from", "offer")?)?;
    let to = pubkey_text(require(offer_map, "to", "offer")?)?;
    if from != pubkey_text(require(accept_map, "from", "accept")?)?
        || to != pubkey_text(require(accept_map, "to", "accept")?)?
    {
        return Err(ContractError::new(
            RefusalCode::IdentityMismatch,
            "migration",
        ));
    }
    if offer.pubkey != from || accept.pubkey != to {
        return Err(ContractError::new(
            RefusalCode::IdentityMismatch,
            "migration signer",
        ));
    }
    let id = text(require(offer_map, "migration", "offer")?, "migration")?;
    if id != text(require(accept_map, "migration", "accept")?, "migration")? {
        return Err(ContractError::new(
            RefusalCode::IdentityMismatch,
            "migration id",
        ));
    }
    if one_tag(accept, "e")? != offer.id {
        return Err(ContractError::new(
            RefusalCode::IdentityMismatch,
            "migration offer",
        ));
    }
    Ok(())
}

/// Grants after a namespace migration. There are none to inherit.
#[must_use]
pub fn grants_after_migration() -> Vec<String> {
    Vec::new()
}

fn parse_listing(event: &Event, object: &Map<String, Value>) -> Result<(), ContractError> {
    reject(
        object,
        &[
            "v",
            "requires",
            "type",
            "package",
            "state",
            "release",
            "title",
            "description",
            "meta",
        ],
        "listing",
    )?;
    let (root, slug) = split_package(&package_id(require(object, "package", "listing")?)?)?;
    if event.pubkey != root || one_tag(event, "d")? != slug {
        return Err(ContractError::new(RefusalCode::IdentityMismatch, "listing"));
    }
    let state = text(require(object, "state", "listing")?, "state")?;
    match (state, object.get("release")) {
        ("hidden", Some(Value::Null)) => Ok(()),
        ("published", Some(value)) => {
            parse_event_ref(value)?;
            Ok(())
        }
        _ => Err(malformed("listing release")),
    }
}

fn parse_release(event: &Event, object: &Map<String, Value>) -> Result<(), ContractError> {
    reject(
        object,
        &[
            "v", "requires", "type", "package", "version", "manifest", "meta",
        ],
        "release",
    )?;
    let (root, _) = split_package(&package_id(require(object, "package", "release")?)?)?;
    if event.pubkey != root {
        return Err(ContractError::new(
            RefusalCode::IdentityMismatch,
            "release signer",
        ));
    }
    let _version = label(require(object, "version", "release")?)?;
    parse_artifact(require(object, "manifest", "release")?)?;
    Ok(())
}

fn parse_revocation(event: &Event, object: &Map<String, Value>) -> Result<(), ContractError> {
    reject(
        object,
        &[
            "v",
            "requires",
            "type",
            "package",
            "release",
            "reason",
            "effective_at",
            "meta",
        ],
        "revocation",
    )?;
    let (root, _) = split_package(&package_id(require(object, "package", "revocation")?)?)?;
    if event.pubkey != root {
        return Err(ContractError::new(
            RefusalCode::IdentityMismatch,
            "revocation signer",
        ));
    }
    let release = parse_event_ref(require(object, "release", "revocation")?)?;
    if one_tag(event, "e")? != release {
        return Err(ContractError::new(
            RefusalCode::IdentityMismatch,
            "revocation",
        ));
    }
    let _reason = text(require(object, "reason", "revocation")?, "reason")?;
    let effective = require(object, "effective_at", "revocation")?
        .as_u64()
        .ok_or_else(|| malformed("effective_at"))?;
    if effective > event.created_at {
        return Err(malformed("effective_at"));
    }
    Ok(())
}

fn parse_migration(event: &Event, object: &Map<String, Value>) -> Result<(), ContractError> {
    reject(
        object,
        &[
            "v",
            "requires",
            "type",
            "from",
            "to",
            "migration",
            "role",
            "offer",
            "meta",
        ],
        "migration",
    )?;
    let from = pubkey_text(require(object, "from", "migration")?)?;
    let _to = pubkey_text(require(object, "to", "migration")?)?;
    let _id = hex_id(require(object, "migration", "migration")?)?;
    let role = text(require(object, "role", "migration")?, "role")?;
    match role {
        "offer" if event.pubkey == from && object.get("offer").is_none() => Ok(()),
        "accept" if event.pubkey != from => {
            parse_event_ref(require(object, "offer", "migration")?)?;
            Ok(())
        }
        _ => Err(ContractError::new(
            RefusalCode::IdentityMismatch,
            "migration role",
        )),
    }
}

fn parse_checkpoint(event: &Event, object: &Map<String, Value>) -> Result<(), ContractError> {
    reject(
        object,
        &[
            "v",
            "requires",
            "type",
            "package",
            "revision",
            "as_of",
            "valid_until",
            "revocations",
            "meta",
        ],
        "checkpoint",
    )?;
    let (root, slug) = split_package(&package_id(require(object, "package", "checkpoint")?)?)?;
    if event.pubkey != root || one_tag(event, "d")? != slug {
        return Err(ContractError::new(
            RefusalCode::IdentityMismatch,
            "checkpoint",
        ));
    }
    let _revision = require(object, "revision", "checkpoint")?
        .as_u64()
        .ok_or_else(|| malformed("revision"))?;
    let as_of = require(object, "as_of", "checkpoint")?
        .as_u64()
        .ok_or_else(|| malformed("as_of"))?;
    let valid_until = require(object, "valid_until", "checkpoint")?
        .as_u64()
        .ok_or_else(|| malformed("valid_until"))?;
    if valid_until <= as_of {
        return Err(malformed("valid_until"));
    }
    match require(object, "revocations", "checkpoint")? {
        Value::Array(items) => {
            let ids = items
                .iter()
                .map(|item| text(item, "revocation").map(str::to_string))
                .collect::<Result<Vec<_>, _>>()?;
            let mut sorted = ids.clone();
            sorted.sort();
            if sorted != ids || sorted.windows(2).any(|pair| pair[0] == pair[1]) {
                return Err(malformed("revocations"));
            }
            Ok(())
        }
        other => {
            parse_artifact(other)?;
            Ok(())
        }
    }
}

fn parse_component(value: &Value, seen: &mut BTreeSet<String>) -> Result<String, ContractError> {
    let object = as_map(value, "component")?;
    reject(
        object,
        &["slug", "kind", "definition", "descriptor"],
        "component",
    )?;
    let slug = slug_text(require(object, "slug", "component")?)?;
    if !seen.insert(slug.clone()) {
        return Err(ContractError::new(RefusalCode::Conflict, "component"));
    }
    let kind = text(require(object, "kind", "component")?, "kind")?;
    if !COMPONENT_KINDS.contains(&kind) {
        return Err(ContractError::new(
            RefusalCode::UnsupportedFeature,
            format!("component kind {kind}"),
        ));
    }
    let definition = parse_artifact(require(object, "definition", "component")?)?;
    if kind == "schema" && definition.media_type != "application/schema+json" {
        return Err(ContractError::new(RefusalCode::Incompatible, "schema"));
    }
    match object.get("descriptor") {
        Some(descriptor) => {
            parse_artifact(descriptor)?;
        }
        None if kind == "guidance" || kind == "schema" => {}
        None => {
            return Err(malformed("descriptor"));
        }
    }
    Ok(definition.digest)
}

fn parse_files(value: &Value) -> Result<Vec<FileEntry>, ContractError> {
    let items = value.as_array().ok_or_else(|| malformed("files"))?;
    let mut files = Vec::new();
    let mut paths = BTreeSet::new();
    let mut folded = BTreeSet::new();
    for item in items {
        let object = as_map(item, "file")?;
        reject(object, &["path", "digest", "size", "media_type"], "file")?;
        let path = normalize_path(text(require(object, "path", "file")?, "path")?)?;
        let fold = path.to_ascii_lowercase();
        if !paths.insert(path.clone()) || !folded.insert(fold) {
            return Err(ContractError::new(RefusalCode::Conflict, "path"));
        }
        let digest = digest_text(require(object, "digest", "file")?)?;
        let size = require(object, "size", "file")?
            .as_u64()
            .ok_or_else(|| malformed("size"))?;
        let media_type = text(require(object, "media_type", "file")?, "media_type")?;
        if media_type == "application/x-symlink" {
            return Err(ContractError::new(RefusalCode::UnsupportedFeature, "link"));
        }
        files.push(FileEntry {
            path,
            digest,
            size,
            media_type: media_type.to_string(),
        });
    }
    Ok(files)
}

fn parse_provenance(value: &Value) -> Result<(), ContractError> {
    let object = as_map(value, "provenance")?;
    reject(object, &["source", "receipts", "unknowns"], "provenance")?;
    let source = text(require(object, "source", "provenance")?, "source")?;
    if source != "local" && source != "prebuilt" {
        return Err(ContractError::new(
            RefusalCode::UnsupportedFeature,
            "provenance",
        ));
    }
    let _receipts = require(object, "receipts", "provenance")?
        .as_array()
        .ok_or_else(|| malformed("receipts"))?;
    let _unknowns = require(object, "unknowns", "provenance")?
        .as_array()
        .ok_or_else(|| malformed("unknowns"))?;
    Ok(())
}

fn parse_hook(value: &Value) -> Result<(), ContractError> {
    let object = as_map(value, "hook")?;
    reject(
        object,
        &[
            "event",
            "operation",
            "input",
            "bounds",
            "on_error",
            "destination",
        ],
        "hook",
    )?;
    let event = text(require(object, "event", "hook")?, "event")?;
    if !matches!(
        event,
        "context.requested" | "operation.completed" | "task.closed"
    ) {
        return Err(ContractError::new(
            RefusalCode::UnsupportedFeature,
            "hook event",
        ));
    }
    let _operation = qualified(require(object, "operation", "hook")?)?;
    parse_schema_ref(require(object, "input", "hook")?)?;
    let _bounds = as_map(require(object, "bounds", "hook")?, "bounds")?;
    let on_error = text(require(object, "on_error", "hook")?, "on_error")?;
    if on_error != "stop" && on_error != "continue" {
        return Err(ContractError::new(
            RefusalCode::UnsupportedFeature,
            "on_error",
        ));
    }
    let destination = text(require(object, "destination", "hook")?, "destination")?;
    if destination != "evidence" && destination != "view" {
        return Err(ContractError::new(
            RefusalCode::UnsupportedFeature,
            "destination",
        ));
    }
    Ok(())
}

fn walk_dependencies(
    id: &str,
    manifests: &BTreeMap<String, Manifest>,
    path: &mut Vec<String>,
) -> Result<(), ContractError> {
    if path.iter().any(|seen| seen == id) {
        return Err(ContractError::new(
            RefusalCode::Incompatible,
            "dependency cycle",
        ));
    }
    let Some(manifest) = manifests.get(id) else {
        return Err(ContractError::new(RefusalCode::ContentUnavailable, id));
    };
    path.push(id.to_string());
    for dependency in &manifest.dependencies {
        walk_dependencies(dependency, manifests, path)?;
    }
    path.pop();
    Ok(())
}

fn parse_event_ref(value: &Value) -> Result<String, ContractError> {
    let object = as_map(value, "event")?;
    reject(object, &["id", "pubkey", "kind", "coordinate"], "event")?;
    let id = hex_id(require(object, "id", "event")?)?;
    let _pubkey = pubkey_text(require(object, "pubkey", "event")?)?;
    let _kind = require(object, "kind", "event")?
        .as_u64()
        .ok_or_else(|| malformed("kind"))?;
    Ok(id)
}

fn version(object: &Map<String, Value>) -> Result<(), ContractError> {
    if object.get("v").and_then(Value::as_u64) != Some(1) {
        return Err(ContractError::new(RefusalCode::UnsupportedVersion, "v"));
    }
    require_empty(object, "requires")
}

fn require_empty(object: &Map<String, Value>, key: &str) -> Result<(), ContractError> {
    let items = require(object, key, "record")?
        .as_array()
        .ok_or_else(|| malformed(key))?;
    if items.is_empty() {
        Ok(())
    } else {
        Err(ContractError::new(RefusalCode::UnsupportedFeature, key))
    }
}

fn one_marker(event: &Event) -> Result<String, ContractError> {
    let markers: Vec<&str> = event
        .tags
        .iter()
        .filter(|tag| tag.name() == Some("t"))
        .filter_map(Tag::value)
        .filter(|value| value.starts_with("oa:ext:"))
        .collect();
    if markers.len() == 1 {
        Ok(markers[0].to_string())
    } else {
        Err(malformed("extension tag"))
    }
}

fn one_tag<'a>(event: &'a Event, name: &str) -> Result<&'a str, ContractError> {
    let values: Vec<&str> = event
        .tags
        .iter()
        .filter(|tag| tag.name() == Some(name))
        .filter_map(Tag::value)
        .collect();
    if values.len() == 1 {
        Ok(values[0])
    } else {
        Err(malformed(name))
    }
}

fn normalize_path(path: &str) -> Result<String, ContractError> {
    if path.starts_with('/') || path.contains('\\') || path.contains('\0') {
        return Err(malformed("path"));
    }
    let mut parts = Vec::new();
    for part in path.split('/') {
        if part.is_empty() || part == "." {
            continue;
        }
        if part == ".." {
            return Err(malformed("path"));
        }
        parts.push(part);
    }
    if parts.is_empty() {
        return Err(malformed("path"));
    }
    Ok(parts.join("/"))
}

fn package_id(value: &Value) -> Result<String, ContractError> {
    let text = text(value, "package")?;
    split_package(text)?;
    Ok(text.to_string())
}

fn split_package(value: &str) -> Result<(String, String), ContractError> {
    let Some((root, slug)) = value.split_once(':') else {
        return Err(malformed("package"));
    };
    if !is_hex(root) || !is_slug(slug) {
        return Err(malformed("package"));
    }
    Ok((root.to_string(), slug.to_string()))
}

fn qualified(value: &Value) -> Result<String, ContractError> {
    let text = text(value, "id")?;
    let Some((root, rest)) = text.split_once(':') else {
        return Err(malformed("id"));
    };
    let Some((package, component)) = rest.split_once('/') else {
        return Err(malformed("id"));
    };
    if !is_hex(root) || !is_slug(package) || !is_slug(component) || component.contains('/') {
        return Err(malformed("id"));
    }
    Ok(text.to_string())
}

fn digest_text(value: &Value) -> Result<String, ContractError> {
    let text = text(value, "digest")?;
    let Some(hex) = text.strip_prefix("sha256:") else {
        return Err(malformed("digest"));
    };
    if !is_hex(hex) {
        return Err(malformed("digest"));
    }
    Ok(text.to_string())
}

fn hex_id(value: &Value) -> Result<String, ContractError> {
    let text = text(value, "id")?;
    if !is_hex(text) {
        return Err(malformed("id"));
    }
    Ok(text.to_string())
}

fn pubkey_text(value: &Value) -> Result<String, ContractError> {
    let text = text(value, "pubkey")?;
    if !is_hex(text) {
        return Err(malformed("pubkey"));
    }
    Ok(text.to_string())
}

fn slug_text(value: &Value) -> Result<String, ContractError> {
    let text = text(value, "slug")?;
    if !is_slug(text) {
        return Err(malformed("slug"));
    }
    Ok(text.to_string())
}

fn label(value: &Value) -> Result<String, ContractError> {
    let text = text(value, "version")?;
    if text.is_empty() || text.chars().any(char::is_whitespace) {
        return Err(malformed("version"));
    }
    Ok(text.to_string())
}

fn string_list(value: &Value, path: &str) -> Result<Vec<String>, ContractError> {
    value
        .as_array()
        .ok_or_else(|| malformed(path))?
        .iter()
        .map(|item| text(item, path).map(str::to_string))
        .collect()
}

fn as_map<'a>(value: &'a Value, path: &str) -> Result<&'a Map<String, Value>, ContractError> {
    value.as_object().ok_or_else(|| malformed(path))
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

fn reject(object: &Map<String, Value>, allowed: &[&str], path: &str) -> Result<(), ContractError> {
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

fn text<'a>(value: &'a Value, path: &str) -> Result<&'a str, ContractError> {
    value.as_str().ok_or_else(|| malformed(path))
}

fn is_hex(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
}

fn is_slug(value: &str) -> bool {
    let mut chars = value.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    value.len() <= 64
        && (first.is_ascii_lowercase() || first.is_ascii_digit())
        && chars.all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '_' || ch == '-')
}

fn malformed(detail: impl Into<String>) -> ContractError {
    ContractError::new(RefusalCode::Malformed, detail)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::contracts::digest_bytes;
    use crate::domain::RelaySigner;
    use serde_json::json;

    fn signer() -> RelaySigner {
        RelaySigner::from_secret_hex(&"11".repeat(32)).unwrap()
    }

    fn sign(kind: u16, tags: Vec<Tag>, body: Value) -> Event {
        signer().sign(1_700_000_000, kind, tags, body.to_string())
    }

    fn marker(kind: &str) -> Tag {
        Tag::new(vec!["t".into(), format!("oa:ext:{kind}:v1")])
    }

    fn artifact(bytes: &[u8]) -> Value {
        json!({
            "digest": digest_bytes(bytes),
            "size": bytes.len(),
            "media_type": "application/json"
        })
    }

    fn manifest_value(package: &str, path: &str, bytes: &[u8]) -> Value {
        json!({
            "v": "openagents.package.v1",
            "requires": [],
            "package": package,
            "version": "1.0.0",
            "license": "MIT",
            "provenance": {"source": "local", "receipts": [], "unknowns": ["builder"]},
            "components": [{
                "slug": "shape",
                "kind": "schema",
                "definition": {
                    "digest": digest_bytes(bytes),
                    "size": bytes.len(),
                    "media_type": "application/schema+json"
                }
            }],
            "files": [{
                "path": path,
                "digest": digest_bytes(bytes),
                "size": bytes.len(),
                "media_type": "application/schema+json"
            }],
            "dependencies": []
        })
    }

    #[test]
    fn a_signed_release_agrees_with_its_tag_and_a_tampered_one_does_not() {
        let root = signer();
        let package = format!("{}:demo", root.pubkey());
        let bytes = br#"{"type":"object"}"#;
        let body = json!({
            "v": 1,
            "requires": [],
            "type": "release",
            "package": package,
            "version": "1.0.0",
            "manifest": artifact(bytes)
        });
        let event = sign(RELEASE_KIND, vec![marker("release")], body);
        parse_record(&event).unwrap();

        let mut tampered = event.clone();
        tampered.content.push(' ');
        assert_eq!(
            parse_record(&tampered).unwrap_err().code,
            RefusalCode::IdentityMismatch
        );

        let mismatched = sign(
            RELEASE_KIND,
            vec![marker("listing")],
            json!({
                "v": 1,
                "requires": [],
                "type": "release",
                "package": package,
                "version": "1.0.0",
                "manifest": artifact(bytes)
            }),
        );
        assert_eq!(
            parse_record(&mismatched).unwrap_err().code,
            RefusalCode::IdentityMismatch
        );
        assert!(execution_pin(RELEASE_KIND));
        assert!(!execution_pin(LISTING_KIND));
        assert!(!execution_pin(CURATION_KIND));
    }

    #[test]
    fn closure_checks_cover_the_refusal_cases() {
        let package = format!("{}:demo", signer().pubkey());
        let bytes = br#"{"type":"object"}"#;
        let manifest = parse_manifest(&manifest_value(&package, "schema.json", bytes)).unwrap();
        let mut files = BTreeMap::new();
        files.insert(digest_bytes(bytes), bytes.to_vec());
        let mut dependencies = BTreeMap::new();
        dependencies.insert("root".into(), manifest.clone());
        verify_closure(&Closure {
            manifest: &manifest,
            files: &files,
            staged: &["schema.json".into()],
            dependencies: &dependencies,
            root: "root",
            byte_limit: 1024,
        })
        .unwrap();

        let extra = verify_closure(&Closure {
            staged: &["schema.json".into(), "payload.bin".into()],
            ..Closure {
                manifest: &manifest,
                files: &files,
                staged: &[],
                dependencies: &dependencies,
                root: "root",
                byte_limit: 1024,
            }
        });
        assert_eq!(extra.unwrap_err().code, RefusalCode::Incompatible);

        assert_eq!(
            equivocation("sha256:aa", "sha256:bb").unwrap_err().code,
            RefusalCode::Conflict
        );
        let mut store = BTreeMap::new();
        store.insert("sha256:other".into(), b"no".to_vec());
        assert_eq!(
            historical_bytes("sha256:pinned", &store).unwrap_err().code,
            RefusalCode::ContentUnavailable
        );
        assert_eq!(
            expand_archive("application/zip").unwrap_err().code,
            RefusalCode::UnsupportedFeature
        );
        let colliding = manifest_value(&package, "Schema.json", bytes);
        let mut second = colliding.clone();
        second["files"].as_array_mut().unwrap().push(json!({
            "path": "schema.json",
            "digest": digest_bytes(bytes),
            "size": bytes.len(),
            "media_type": "application/json"
        }));
        assert_eq!(
            parse_manifest(&second).unwrap_err().code,
            RefusalCode::Conflict
        );
        let escaped = manifest_value(&package, "../secret", bytes);
        assert_eq!(
            parse_manifest(&escaped).unwrap_err().code,
            RefusalCode::Malformed
        );

        let mut cycled = manifest.clone();
        cycled.dependencies.push("other".into());
        let mut other = manifest.clone();
        other.dependencies.push("root".into());
        let mut graph = BTreeMap::new();
        graph.insert("root".into(), cycled);
        graph.insert("other".into(), other);
        assert_eq!(
            verify_closure(&Closure {
                manifest: graph.get("root").unwrap(),
                files: &files,
                staged: &["schema.json".into()],
                dependencies: &graph,
                root: "root",
                byte_limit: 1024,
            })
            .unwrap_err()
            .code,
            RefusalCode::Incompatible
        );
    }

    #[test]
    fn publication_install_and_revocation_stay_distinct() {
        let human = authorize(&Authorization {
            surface: Surface::Human,
            phase: Phase::Publish,
            side: Side::Script,
            authorized: true,
        });
        let model = authorize(&Authorization {
            surface: Surface::Model,
            phase: Phase::Install,
            side: Side::Probe,
            authorized: true,
        });
        assert_eq!(human.unwrap_err().code, model.unwrap_err().code);
        assert!(
            authorize(&Authorization {
                surface: Surface::Human,
                phase: Phase::Publish,
                side: Side::None,
                authorized: true,
            })
            .is_ok()
        );

        let staged = transition(
            &Install::Absent,
            InstallStep::Stage {
                lock: "lock-a".into(),
                verified: true,
            },
        )
        .unwrap();
        assert!(matches!(staged, Install::Staged { .. }));
        let installed = transition(&staged, InstallStep::Commit).unwrap();
        let rolled = transition(
            &installed,
            InstallStep::Rollback {
                previous: "lock-old".into(),
                eligible: true,
            },
        )
        .unwrap();
        assert_eq!(
            rolled,
            Install::Installed {
                lock: "lock-old".into()
            }
        );
        assert_eq!(preserve_active_pin("lock-a", "lock-b"), "lock-a");

        let mut knowledge = RevocationKnowledge {
            revision: 1,
            revocations: BTreeSet::from(["a".into()]),
        };
        knowledge = absorb(&knowledge, 2, &["a".into(), "b".into()]).unwrap();
        assert!(knowledge.revocations.contains("b"));
        assert_eq!(
            absorb(&knowledge, 2, &["a".into()]).unwrap_err().code,
            RefusalCode::Conflict
        );
        assert_eq!(
            absorb(&knowledge, 1, &["a".into(), "b".into()])
                .unwrap_err()
                .code,
            RefusalCode::Stale
        );
        assert_eq!(
            fresh(&Freshness {
                as_of: 10,
                valid_until: 20,
                now: 15,
                skew: 5,
                present: false,
                empty_answer: false,
                strict: true,
                explicit_pin: false,
            })
            .unwrap_err()
            .code,
            RefusalCode::Stale
        );
        fresh(&Freshness {
            as_of: 10,
            valid_until: 20,
            now: 15,
            skew: 5,
            present: false,
            empty_answer: false,
            strict: true,
            explicit_pin: true,
        })
        .unwrap();
        assert_eq!(
            fresh(&Freshness {
                as_of: 10,
                valid_until: 20,
                now: 15,
                skew: 5,
                present: true,
                empty_answer: true,
                strict: false,
                explicit_pin: true,
            })
            .unwrap_err()
            .code,
            RefusalCode::ContentUnavailable
        );
    }

    #[test]
    fn migration_needs_both_roots_and_does_not_carry_grants() {
        let old = signer();
        let new = RelaySigner::from_secret_hex(&"22".repeat(32)).unwrap();
        let migration = "ab".repeat(32);
        let offer = old.sign(
            1_700_000_000,
            MIGRATION_KIND,
            vec![marker("migration")],
            json!({
                "v": 1,
                "requires": [],
                "type": "migration",
                "from": old.pubkey(),
                "to": new.pubkey(),
                "migration": migration,
                "role": "offer"
            })
            .to_string(),
        );
        let accept = new.sign(
            1_700_000_001,
            MIGRATION_KIND,
            vec![
                marker("migration"),
                Tag::new(vec!["e".into(), offer.id.clone()]),
            ],
            json!({
                "v": 1,
                "requires": [],
                "type": "migration",
                "from": old.pubkey(),
                "to": new.pubkey(),
                "migration": migration,
                "role": "accept",
                "offer": {"id": offer.id, "pubkey": old.pubkey(), "kind": MIGRATION_KIND}
            })
            .to_string(),
        );
        pair_migration(&offer, &accept).unwrap();
        assert!(grants_after_migration().is_empty());

        let spoof = old.sign(
            1_700_000_001,
            MIGRATION_KIND,
            vec![
                marker("migration"),
                Tag::new(vec!["e".into(), offer.id.clone()]),
            ],
            json!({
                "v": 1,
                "requires": [],
                "type": "migration",
                "from": old.pubkey(),
                "to": new.pubkey(),
                "migration": migration,
                "role": "accept",
                "offer": {"id": offer.id, "pubkey": old.pubkey(), "kind": MIGRATION_KIND}
            })
            .to_string(),
        );
        assert_eq!(
            pair_migration(&offer, &spoof).unwrap_err().code,
            RefusalCode::IdentityMismatch
        );
    }

    #[test]
    fn a_locator_must_match_and_descriptors_refuse_hidden_effects() {
        let bytes = br#"{"type":"object"}"#;
        let artifact_value = json!({
            "digest": digest_bytes(bytes),
            "size": bytes.len(),
            "media_type": "application/schema+json"
        });
        let artifact = parse_artifact(&artifact_value).unwrap();
        let hash = digest_bytes(bytes)
            .trim_start_matches("sha256:")
            .to_string();
        let locator = sign(
            LOCATOR_KIND,
            vec![
                Tag::new(vec!["m".into(), "application/schema+json".into()]),
                Tag::new(vec!["x".into(), hash]),
                Tag::new(vec!["size".into(), bytes.len().to_string()]),
            ],
            json!({}),
        );
        locator_matches(&locator, &artifact).unwrap();
        let mut wrong = locator.clone();
        wrong.tags[0] = Tag::new(vec!["m".into(), "text/plain".into()]);
        // The signature no longer covers the tags.
        assert_eq!(
            locator_matches(&wrong, &artifact).unwrap_err().code,
            RefusalCode::IdentityMismatch
        );

        let package = format!("{}:demo", signer().pubkey());
        let id = format!("{package}/read");
        let operation = json!({
            "v": "openagents.operation.v1",
            "requires": [],
            "id": id,
            "kind": "operation",
            "definition": artifact_value,
            "summary": "Read a record.",
            "input": artifact_value_schema(bytes),
            "output": artifact_value_schema(bytes),
            "preconditions": {"formats": [], "evidence": []},
            "effects": {},
            "minimum": {},
            "guidance": []
        });
        // input/output must be schema media type. artifact_value is application/json.
        // Use schema media type below.
        let _ = operation;
        let schema = artifact_value_schema(bytes);
        parse_operation(
            &json!({
                "v": "openagents.operation.v1",
                "requires": [],
                "id": id,
                "kind": "operation",
                "definition": schema,
                "summary": "Read a record.",
                "input": schema,
                "output": schema,
                "preconditions": {"formats": [], "evidence": []},
                "effects": {},
                "minimum": {},
                "guidance": []
            }),
            &id,
        )
        .unwrap();

        let function_id = format!("{package}/judge");
        let builder = format!("{package}/build");
        parse_function(&json!({
            "v": "openagents.function.v1",
            "requires": [],
            "id": function_id,
            "input": schema,
            "output": schema,
            "state_builder": builder,
            "questions": schema,
            "policy": format!("{package}/policy"),
            "model_requirements": {"apis": ["responses"], "schemas": ["answer"], "requires_scorable_answer": false},
            "bounds": {},
            "evaluation": []
        }))
        .unwrap();
        assert_eq!(
            parse_function(&json!({
                "v": "openagents.function.v1",
                "requires": [],
                "id": function_id,
                "input": schema,
                "output": schema,
                "state_builder": function_id,
                "questions": schema,
                "policy": format!("{package}/policy"),
                "model_requirements": {"apis": [], "schemas": [], "requires_scorable_answer": false},
                "bounds": {},
                "evaluation": []
            }))
            .unwrap_err()
            .code,
            RefusalCode::Incompatible
        );

        parse_skill(&json!({
            "v": "openagents.skill.v1",
            "requires": [],
            "id": format!("{package}/notes"),
            "description": "Notes.",
            "body": schema,
            "applicability": ["text"],
            "allowed_operations": [],
            "lifetime": "task",
            "hooks": [{
                "event": "task.closed",
                "operation": format!("{package}/read"),
                "input": schema,
                "bounds": {},
                "on_error": "stop",
                "destination": "evidence"
            }]
        }))
        .unwrap();
        assert_eq!(
            parse_skill(&json!({
                "v": "openagents.skill.v1",
                "requires": [],
                "id": format!("{package}/notes"),
                "description": "Notes.",
                "body": schema,
                "applicability": [],
                "allowed_operations": [],
                "lifetime": "task",
                "hooks": [{
                    "event": "shell.exec",
                    "operation": format!("{package}/read"),
                    "input": schema,
                    "bounds": {},
                    "on_error": "stop",
                    "destination": "evidence"
                }]
            }))
            .unwrap_err()
            .code,
            RefusalCode::UnsupportedFeature
        );
    }

    fn artifact_value_schema(bytes: &[u8]) -> Value {
        json!({
            "digest": digest_bytes(bytes),
            "size": bytes.len(),
            "media_type": "application/schema+json"
        })
    }
}
