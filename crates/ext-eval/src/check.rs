//! Checking someone's published result: fetch its suite, rerun it against
//! the same subject, and publish a confirm or a dispute.
//!
//! A check starts from a `3189` result. [`materialize`] reads the suite's
//! NIP-EXT release, fetches every file its manifest lists (the caller
//! supplies the fetch, a Blossom server in practice), verifies each
//! against its digest and the package against
//! `nostr::eval_ext::check_suite_package`, and writes the cases out as the
//! same eval directory layout the runner reads. [`same_subject`] refuses a
//! local extension whose package record or run lock differs from the
//! report's: a check reruns the same bytes or nothing. After the rerun,
//! the caller publishes a `3189` citing the original with the `check`
//! marker, and [`linkage`] says whether it confirms or disputes.

use std::path::{Path, PathBuf};

use nostr::domain::Event;
use nostr::eval_ext::{Linkage, Publication};
use serde_json::Value;

/// What went wrong checking a result.
#[derive(Debug, thiserror::Error)]
pub enum CheckError {
    /// A shared contract refused a record.
    #[error("{0}")]
    Contract(String),
    /// A file couldn't be fetched or written.
    #[error("{0}")]
    Io(String),
    /// The local extension is not the subject the result measured.
    #[error("{0}")]
    NotTheSubject(String),
}

fn contract(error: nostr::contracts::ContractError) -> CheckError {
    CheckError::Contract(error.to_string())
}

/// A published suite written out for the runner.
#[derive(Clone, Debug)]
pub struct Materialized {
    /// The eval directory: `<into>/evals`.
    pub eval_dir: PathBuf,
    /// The suite author's public key.
    pub author: String,
    /// The suite's package slug.
    pub package: String,
    /// The suite component's slug.
    pub component: String,
    /// The release's `{id, pubkey, kind}` EventRef.
    pub release: Value,
    /// The suite document's digest.
    pub suite_digest: String,
}

/// Reads a published result and checks it under the profile.
///
/// # Errors
///
/// Returns [`CheckError::Contract`] when the event isn't a valid result.
pub fn read_result(event: &Event) -> Result<Publication, CheckError> {
    nostr::eval_ext::parse_publication(event).map_err(contract)
}

/// Writes the suite `release` publishes into `into`, fetching each file
/// with `fetch(digest)` and checking every byte.
///
/// # Errors
///
/// Returns [`CheckError`] when the release, its manifest, or a file
/// doesn't check, or a file can't be fetched or written.
pub fn materialize(
    release: &Event,
    fetch: &dyn Fn(&str) -> Result<Vec<u8>, String>,
    into: &Path,
) -> Result<Materialized, CheckError> {
    let body = nostr::ext::parse_record(release).map_err(contract)?;
    let manifest_ref =
        nostr::contracts::parse_artifact(body.get("manifest").unwrap_or(&Value::Null))
            .map_err(contract)?;
    let manifest_bytes = fetch(&manifest_ref.digest).map_err(CheckError::Io)?;
    nostr::eval_ext::parse_release(release, &manifest_bytes).map_err(contract)?;
    let manifest: Value = serde_json::from_slice(&manifest_bytes)
        .map_err(|error| CheckError::Contract(error.to_string()))?;
    let definition = manifest
        .get("components")
        .and_then(Value::as_array)
        .and_then(|components| {
            components.iter().find(|component| {
                component.get("kind").and_then(Value::as_str)
                    == Some(nostr::eval_ext::COMPONENT_KIND)
            })
        })
        .and_then(|component| component.get("definition"))
        .ok_or_else(|| CheckError::Contract("the release holds no eval-suite component".into()))?;
    let suite_ref = nostr::contracts::parse_artifact(definition).map_err(contract)?;
    let suite = fetch(&suite_ref.digest).map_err(CheckError::Io)?;
    let parsed = nostr::eval_ext::parse_suite(&suite).map_err(contract)?;
    let cases = fetch(&parsed.cases.digest).map_err(CheckError::Io)?;
    let package =
        nostr::eval_ext::check_suite_package(&manifest, &suite, &cases).map_err(contract)?;
    let write = |path: &Path, bytes: &[u8]| -> Result<(), CheckError> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|error| CheckError::Io(error.to_string()))?;
        }
        std::fs::write(path, bytes).map_err(|error| CheckError::Io(error.to_string()))
    };
    for file in &package.manifest.files {
        let Some(relative) = file.path.strip_prefix("evals/") else {
            continue;
        };
        let bytes = fetch(&file.digest).map_err(CheckError::Io)?;
        if bytes.len() as u64 != file.size {
            return Err(CheckError::Contract(format!(
                "{} is {} bytes, and the manifest says {}",
                file.path,
                bytes.len(),
                file.size
            )));
        }
        let target = into.join("evals").join(relative);
        if !target.starts_with(into.join("evals")) {
            return Err(CheckError::Contract(format!(
                "{} leaves the eval directory",
                file.path
            )));
        }
        write(&target, &bytes)?;
    }
    let (author, rest) = parsed
        .id
        .split_once(':')
        .ok_or_else(|| CheckError::Contract("the suite ID is not qualified".into()))?;
    let (suite_package, component) = rest
        .split_once('/')
        .ok_or_else(|| CheckError::Contract("the suite ID names no component".into()))?;
    Ok(Materialized {
        eval_dir: into.join("evals"),
        author: author.to_string(),
        package: suite_package.to_string(),
        component: component.to_string(),
        release: serde_json::json!({
            "id": release.id,
            "pubkey": release.pubkey,
            "kind": release.kind,
        }),
        suite_digest: suite_ref.digest,
    })
}

/// Refuses a local subject whose DefinitionRef or run lock differs from
/// the ones `original` measured.
///
/// # Errors
///
/// Returns [`CheckError::NotTheSubject`] naming what differs.
pub fn same_subject(
    original: &Publication,
    definition: &Value,
    lock: &[u8],
) -> Result<(), CheckError> {
    let local = nostr::contracts::parse_definition(definition).map_err(contract)?;
    let measured = &original.report.subject.definition;
    if local.id != measured.id
        || local.artifact.digest != measured.artifact.digest
        || local.artifact.size != measured.artifact.size
    {
        return Err(CheckError::NotTheSubject(format!(
            "this extension is {} ({}), and the result measured {} ({})",
            local.id, local.artifact.digest, measured.id, measured.artifact.digest
        )));
    }
    let lock_digest = nostr::contracts::digest_bytes(lock);
    if lock_digest != original.report.subject.lock.digest {
        return Err(CheckError::NotTheSubject(format!(
            "this extension's run lock is {lock_digest}, and the result held {}; a check \
             reruns the same programs, skills, and agent binary",
            original.report.subject.lock.digest
        )));
    }
    Ok(())
}

/// Whether `check` confirms or disputes `original`.
#[must_use]
pub fn linkage(original: &Publication, check: &Publication) -> Linkage {
    nostr::eval_ext::linkage(original, check)
}

/// The word a check's outcome is shown as.
#[must_use]
pub const fn linkage_word(linkage: Linkage) -> &'static str {
    match linkage {
        Linkage::Confirm => "confirm",
        Linkage::Dispute => "dispute",
        Linkage::NotACheck => "not_a_check",
    }
}
