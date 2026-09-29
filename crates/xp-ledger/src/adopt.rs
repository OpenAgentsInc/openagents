//! Adoption into Coder's defaults: the `openagents:coder-defaults` package
//! record, and the documents an operator's decision is made of.
//!
//! Adopting a tool takes three signed or digested records, all built
//! here as pure functions: an `openagents.eval-admission.v1` decision that
//! cites the tool's confirmed reports, the next `coder-defaults` manifest,
//! which depends on the tool's release and cites the decision in its
//! provenance, and the NIP-EXT release (`3184`) that pins the manifest.
//! The operator's command signs and publishes them; the referee then
//! checks `eval-adopt` against them and signs its awards. A candidate is
//! never adopted automatically.

use nostr::contracts::{self, ContractError};
use nostr::domain::Tag;
use nostr::eval_ext::{self, ADMISSION_SCHEMA};
use nostr::kb::Unsigned;
use nostr::{ext, kinds};
use serde_json::{Value, json};

use crate::eval::DEFAULTS_SLUG;

/// The package record, `packages/coder-defaults/package.json`.
pub const PACKAGE_RECORD: &str = include_str!("../../../packages/coder-defaults/package.json");

/// The adoption policy an admission names, `packages/coder-defaults/policy.md`.
pub const POLICY: &str = include_str!("../../../packages/coder-defaults/policy.md");

/// The scope an admission names, `packages/coder-defaults/scope.json`.
pub const SCOPE: &str = include_str!("../../../packages/coder-defaults/scope.json");

/// The manifest schema of a NIP-EXT package.
pub const PACKAGE_SCHEMA: &str = "openagents.package.v1";

/// The `coder-defaults` root key, hex, from the package record.
///
/// # Panics
///
/// Never for the checked-in record; a test checks it.
#[must_use]
pub fn root() -> String {
    let record: Value = serde_json::from_str(PACKAGE_RECORD).expect("the package record is JSON");
    record["root"]
        .as_str()
        .expect("the package record names its root")
        .to_owned()
}

/// `<root>:coder-defaults`.
#[must_use]
pub fn package() -> String {
    package_of(&root())
}

/// `<root>:coder-defaults` for another root, such as a test's.
#[must_use]
pub fn package_of(root: &str) -> String {
    format!("{root}:{DEFAULTS_SLUG}")
}

fn tag(values: &[&str]) -> Tag {
    Tag::new(values.iter().map(|v| (*v).to_owned()).collect())
}

fn artifact(bytes: &[u8], media_type: &str, schema: Option<&str>) -> Value {
    let mut value = json!({
        "digest": contracts::digest_bytes(bytes),
        "size": bytes.len(),
        "media_type": media_type,
    });
    if let Some(schema) = schema {
        value["schema"] = json!(schema);
    }
    value
}

/// An `openagents.eval-admission.v1` decision to `admit` the subject the
/// cited results tested, issued by `root`, as its exact bytes. `results`
/// are the results' publications; each must be on the same subject.
///
/// # Errors
///
/// When there's no result, a result isn't a valid publication, the
/// results tested different subjects, or the document doesn't parse back.
pub fn admission(
    root: &str,
    results: &[&nostr::domain::Event],
    expires_at: u64,
) -> Result<Vec<u8>, String> {
    let mut reports = Vec::new();
    let mut subject: Option<Value> = None;
    for result in results {
        let (report, definition) = crate::eval::cited(result)?;
        if subject.as_ref().is_some_and(|s| *s != definition) {
            return Err("the cited results tested different subjects".into());
        }
        subject = Some(definition);
        reports.push(report);
    }
    let subject = subject.ok_or("an admission cites at least one result")?;
    let value = json!({
        "v": ADMISSION_SCHEMA,
        "requires": [],
        "subject": subject,
        "reports": reports,
        "policy": {
            "id": format!("{root}:{DEFAULTS_SLUG}/policy"),
            "artifact": artifact(POLICY.as_bytes(), "text/markdown", None),
        },
        "scope": artifact(SCOPE.as_bytes(), "application/json", None),
        "decision": "admit",
        "issuer": root,
        "expires_at": expires_at,
    });
    let bytes = contracts::jcs(&value).map_err(|e| e.to_string())?;
    eval_ext::parse_admission(&bytes).map_err(|e| e.to_string())?;
    Ok(bytes)
}

/// The next `coder-defaults` manifest, as its exact bytes: `dependencies`
/// (release IDs, the previous release's plus the adopted one) and
/// `receipts` (the admissions' ArtifactRefs, the previous release's plus
/// the new one).
///
/// # Errors
///
/// When the manifest doesn't parse back.
pub fn manifest(
    package: &str,
    version: &str,
    dependencies: &[String],
    receipts: &[Value],
) -> Result<Vec<u8>, String> {
    let value = json!({
        "v": PACKAGE_SCHEMA,
        "requires": [],
        "package": package,
        "version": version,
        "license": "CC0-1.0",
        "provenance": {
            "source": "local",
            "receipts": receipts,
            "unknowns": ["An admission records an operator's decision; it is not a security review."],
        },
        "components": [],
        "files": [],
        "dependencies": dependencies,
    });
    let bytes = contracts::jcs(&value).map_err(|e| e.to_string())?;
    ext::parse_manifest(&value).map_err(|e| e.to_string())?;
    Ok(bytes)
}

/// An admission's bytes as the ArtifactRef a manifest cites.
#[must_use]
pub fn receipt(admission: &[u8]) -> Value {
    artifact(admission, "application/json", Some(ADMISSION_SCHEMA))
}

/// The receipts a manifest's provenance cites, as JSON.
///
/// # Errors
///
/// When the bytes aren't JSON.
pub fn receipts_of(manifest: &[u8]) -> Result<Vec<Value>, String> {
    let value: Value = serde_json::from_slice(manifest).map_err(|e| e.to_string())?;
    Ok(value
        .pointer("/provenance/receipts")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default())
}

/// The parts of the NIP-EXT release (`3184`) that pins `manifest`. The
/// signer must be the package's root.
///
/// # Errors
///
/// When the manifest doesn't parse.
pub fn release(manifest: &[u8]) -> Result<Unsigned, ContractError> {
    let value: Value = serde_json::from_slice(manifest)
        .map_err(|_| ContractError::new(contracts::RefusalCode::Malformed, "manifest"))?;
    let parsed = ext::parse_manifest(&value)?;
    let body = json!({
        "v": 1, "requires": [], "type": "release",
        "package": parsed.package,
        "version": parsed.version,
        "manifest": artifact(manifest, "application/json", Some(PACKAGE_SCHEMA)),
    });
    Ok(Unsigned {
        kind: kinds::EXT_RELEASE,
        tags: vec![tag(&["t", "oa:ext:release:v1"])],
        content: body.to_string(),
    })
}

/// The parts of a NIP-94 locator (`1063`) that says where `bytes` can be
/// fetched.
#[must_use]
pub fn locator(bytes: &[u8], url: &str) -> Unsigned {
    let digest = contracts::digest_bytes(bytes);
    Unsigned {
        kind: ext::LOCATOR_KIND,
        tags: vec![
            tag(&["url", url]),
            tag(&["x", digest.trim_start_matches("sha256:")]),
            tag(&["m", "application/json"]),
            tag(&["size", &bytes.len().to_string()]),
        ],
        content: "A coder-defaults document, pinned by its digest.".into(),
    }
}

/// Where a `coder-defaults` document is kept in the repository, by digest:
/// `packages/coder-defaults/documents/<hex>.json`.
#[must_use]
pub fn document_path(bytes: &[u8]) -> String {
    let digest = contracts::digest_bytes(bytes);
    format!(
        "packages/coder-defaults/documents/{}.json",
        digest.trim_start_matches("sha256:")
    )
}

/// The public URL a locator names for a document.
#[must_use]
pub fn document_url(bytes: &[u8]) -> String {
    format!(
        "https://raw.githubusercontent.com/OpenAgentsInc/openagents/main/{}",
        document_path(bytes)
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_package_record_names_a_root_and_its_documents_parse() {
        let root = root();
        assert!(root.len() == 64 && root.bytes().all(|b| b.is_ascii_hexdigit()));
        assert_eq!(package(), format!("{root}:coder-defaults"));
        let scope: Value = serde_json::from_str(SCOPE).unwrap();
        assert!(scope.is_object());
        assert!(POLICY.contains("three"));
        let bytes = manifest(&package(), "1", &[], &[]).unwrap();
        release(&bytes).unwrap();
    }
}
