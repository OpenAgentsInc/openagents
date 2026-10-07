//! Read-only verification of explicitly selected private Brainstorm pilot files.

use crate::Args;
use brainstorm_client::{Algorithm, Attribution, Completeness, Observation};
use receipts::brainstorm_pilot::{
    self as pilot, Coverage, Lookup, Operation, Pilot, Reference, Response,
};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::io::Read;
use std::path::{Path, PathBuf};

const USAGE: &str =
    "openagents plugin brainstorm-pilot check --input PRIVATE_RECORD --sources PRIVATE_DIRECTORY";

pub(crate) fn check(words: &[String]) -> Result<Value, String> {
    let args = Args::parse(words, &[])?;
    if args.positional() != ["check"]
        || args
            .option_names()
            .iter()
            .any(|name| !["input", "sources"].contains(name))
    {
        return Err(USAGE.into());
    }
    let required = |name| {
        args.option(name)
            .filter(|v| !v.is_empty())
            .ok_or_else(|| USAGE.to_owned())
    };
    verify(
        Path::new(required("input")?),
        Path::new(required("sources")?),
        atif::now_ms(),
    )
}

fn verify(input: &Path, sources: &Path, now: u64) -> Result<Value, String> {
    private_directory(sources)?;
    let bytes = private_file(input, pilot::MAX_RECORD_BYTES)?;
    let record: Pilot =
        serde_json::from_slice(&bytes).map_err(|_| "Invalid Brainstorm pilot JSON.".to_owned())?;
    let summary = record.validate(now)?;
    // Validate the curve point, not merely hex syntax. No signer is loaded.
    let key = nostr::nip19::decode_npub(&nostr::nip19::encode_npub(&hex_bytes(
        &record.target_pubkey,
    )?))
    .map_err(|_| "Pilot target is not a valid public key.".to_owned())?;
    if key.iter().map(|b| format!("{b:02x}")).collect::<String>() != record.target_pubkey {
        return Err("Pilot target must use its exact canonical public key.".into());
    }
    let mut files = BTreeMap::new();
    let mut total = 0usize;
    for reference in record.references() {
        if let Some((digest, _)) = files.get(&reference.path) {
            if digest != &reference.sha256 {
                return Err("Conflicting evidence digests for one path.".into());
            }
            continue;
        }
        let path = source_path(sources, reference)?;
        let data = private_file(&path, pilot::MAX_SOURCE_BYTES)?;
        total = total.saturating_add(data.len());
        if total > 2 * 1024 * 1024 {
            return Err("Pilot sources exceed 2 MiB.".into());
        }
        if sha256(&data) != reference.sha256 {
            return Err("Pilot evidence does not match its pinned digest.".into());
        }
        files.insert(reference.path.clone(), (reference.sha256.clone(), data));
    }
    let data = |reference: &Reference| &files[&reference.path].1;
    for lookup in &record.lookups {
        let observation = complete_observation(data(&lookup.observation))?;
        if project(lookup, &record.target_pubkey, &observation)? != *lookup {
            return Err("Lookup projection differs from its pinned exact-key observation.".into());
        }
    }
    for link in &record.approved_public_links {
        public_url(link)?;
    }
    if let Some(reference) = &record.profile_event {
        let event: nostr::domain::Event = serde_json::from_slice(data(reference))
            .map_err(|_| "Profile evidence must be one signed Nostr event.".to_owned())?;
        event
            .validate_nip01_structure()
            .map_err(|_| "Profile evidence has an invalid Nostr wire structure.".to_owned())?;
        event
            .validate_crypto()
            .map_err(|_| "Profile evidence has an invalid signature.".to_owned())?;
        if event.kind != 0 || event.pubkey != record.target_pubkey {
            return Err("Profile evidence must be kind 0 signed by the exact pilot target.".into());
        }
        let content: Value = serde_json::from_str(&event.content)
            .map_err(|_| "Invalid profile content.".to_owned())?;
        if !content.is_object()
            || record
                .approved_public_links
                .iter()
                .any(|link| !has_exact_link(&content, link))
        {
            return Err(
                "Profile evidence must contain every separately approved public link.".into(),
            );
        }
    }
    let record_digest = sha256(&bytes);
    Ok(json!({
        "text": "Checked private pilot source pins and exact-key projections. Funnel rows remain fixture or operator claims; paid conversion and activation are not independently verified.",
        "record_sha256": record_digest, "verified_source_files": files.len(), "summary": summary,
    }))
}

fn complete_observation(bytes: &[u8]) -> Result<Observation, String> {
    let invalid = || {
        "Lookup source must be the complete normalized Brainstorm observation without context projection fields.".to_owned()
    };
    let value: Value = serde_json::from_slice(bytes).map_err(|_| invalid())?;
    let fields = value.as_object().ok_or_else(invalid)?;
    let expected = [
        "operation",
        "configuration",
        "configuration_digest",
        "input_digest",
        "house",
        "subjects",
        "responses",
        "enrichment_error",
        "completeness",
        "expires_at_ms",
    ];
    if fields.len() != expected.len() || expected.iter().any(|name| !fields.contains_key(*name)) {
        return Err(invalid());
    }
    serde_json::from_value(value).map_err(|_| invalid())
}

fn public_url(value: &str) -> Result<(), String> {
    let url = reqwest::Url::parse(value).map_err(|_| "Invalid public HTTPS URL.".to_owned())?;
    if url.scheme() != "https"
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
    {
        return Err("Public links must use HTTPS without credentials.".into());
    }
    Ok(())
}

fn has_exact_link(value: &Value, link: &str) -> bool {
    match value {
        Value::String(text) => text.split_whitespace().any(|word| word == link),
        Value::Array(items) => items.iter().any(|item| has_exact_link(item, link)),
        Value::Object(fields) => fields.values().any(|item| has_exact_link(item, link)),
        _ => false,
    }
}

fn sha256(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn project(template: &Lookup, target: &str, observation: &Observation) -> Result<Lookup, String> {
    public_url(&observation.configuration.origin)?;
    let origin = reqwest::Url::parse(&observation.configuration.origin)
        .map_err(|_| "Invalid observation origin.".to_owned())?;
    if origin.path() != "/"
        || origin.query().is_some()
        || origin.fragment().is_some()
        || observation.house.origin != observation.configuration.origin
        || observation.house.attribution != Attribution::SeparateHttpsObservation
        || observation
            .responses
            .iter()
            .any(|r| r.origin != observation.configuration.origin)
    {
        return Err("Observation provenance must retain the configured origin and separate HTTPS house attribution.".into());
    }
    observation
        .configuration
        .limits
        .validate()
        .map_err(|_| "Invalid observation limits.".to_owned())?;
    if serde_json::to_vec(observation)
        .map_err(|_| "Invalid observation.".to_owned())?
        .len()
        > 64 * 1024
        || observation.subjects.len()
            > if observation.operation == brainstorm_client::Operation::SearchPeople {
                10
            } else {
                20
            }
    {
        return Err("Observation exceeds native output bounds.".into());
    }
    let found: Vec<_> = observation
        .subjects
        .iter()
        .filter(|s| s.pubkey == target)
        .collect();
    if found.len() > 1 {
        return Err("Observation repeats the exact target.".into());
    }
    let subject = found.first();
    let coverage = match subject {
        None => Coverage::Absent,
        Some(subject) => match &subject.influence {
            None => Coverage::Unavailable,
            Some(influence) => match influence.coverage {
                brainstorm_client::Coverage::Reported => Coverage::Reported,
                brainstorm_client::Coverage::Unknown => Coverage::Unknown,
            },
        },
    };
    Ok(Lookup {
        id: template.id.clone(),
        recorded_at_ms: template.recorded_at_ms,
        observation: template.observation.clone(),
        operation: match observation.operation {
            brainstorm_client::Operation::SearchPeople => Operation::SearchPeople,
            brainstorm_client::Operation::Rank => Operation::Rank,
        },
        origin: observation.configuration.origin.clone(),
        configuration_digest: observation.configuration_digest.clone(),
        input_digest: observation.input_digest.clone(),
        house_pubkey: observation.house.pubkey.clone(),
        house_discovered_at_ms: observation.house.discovered_at_ms,
        expires_at_ms: observation.expires_at_ms,
        partial: observation.completeness == Completeness::Partial,
        relevance: subject.and_then(|s| s.relevance),
        influence: subject.and_then(|s| s.influence.as_ref().map(|n| n.value)),
        coverage,
        responses: observation
            .responses
            .iter()
            .map(|r| Response {
                endpoint: r.endpoint.clone(),
                status: r.status,
                algorithm: r.requested_algorithm.map(|a| match a {
                    Algorithm::Relevance => "relevance".into(),
                    Algorithm::Graperank => "graperank".into(),
                }),
                fetched_at_ms: r.fetched_at_ms,
                expires_at_ms: r.expires_at_ms,
                input_digest: r.input_digest.clone(),
                output_digest: r.output_digest.clone(),
            })
            .collect(),
    })
}

fn hex_bytes(value: &str) -> Result<[u8; 32], String> {
    let mut bytes = [0; 32];
    for (index, byte) in bytes.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&value[index * 2..index * 2 + 2], 16)
            .map_err(|_| "Invalid public key.".to_owned())?;
    }
    Ok(bytes)
}

fn source_path(root: &Path, reference: &Reference) -> Result<PathBuf, String> {
    reference.validate()?;
    let mut path = root.to_path_buf();
    let parts: Vec<_> = reference.path.split('/').collect();
    for (index, part) in parts.iter().enumerate() {
        path.push(part);
        let metadata = std::fs::symlink_metadata(&path)
            .map_err(|_| "Pilot evidence is unavailable.".to_owned())?;
        if metadata.file_type().is_symlink() || index + 1 < parts.len() && !metadata.is_dir() {
            return Err("Pilot sources cannot traverse links or non-directories.".into());
        }
        private_mode(&metadata)?;
    }
    Ok(path)
}

fn private_directory(path: &Path) -> Result<(), String> {
    let metadata = std::fs::symlink_metadata(path)
        .map_err(|_| "Private pilot source directory is unavailable.".to_owned())?;
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        return Err("Pilot sources need an explicit private directory without links.".into());
    }
    private_mode(&metadata)
}

fn private_file(path: &Path, limit: usize) -> Result<Vec<u8>, String> {
    let metadata = std::fs::symlink_metadata(path)
        .map_err(|_| "Private pilot file is unavailable.".to_owned())?;
    if !metadata.is_file() || metadata.file_type().is_symlink() || metadata.len() > limit as u64 {
        return Err("Pilot evidence needs a bounded regular file without links.".into());
    }
    private_mode(&metadata)?;
    let mut bytes = Vec::new();
    let mut options = std::fs::OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK);
    }
    let file = options
        .open(path)
        .map_err(|_| "Pilot evidence is unreadable.".to_owned())?;
    let opened = file
        .metadata()
        .map_err(|_| "Pilot evidence is unreadable.".to_owned())?;
    if !opened.is_file() || opened.len() > limit as u64 {
        return Err("Pilot evidence needs a bounded regular file.".into());
    }
    private_mode(&opened)?;
    file.take(limit as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| "Pilot evidence is unreadable.".to_owned())?;
    if bytes.is_empty() || bytes.len() > limit {
        return Err("Pilot evidence is empty or exceeds its byte limit.".into());
    }
    Ok(bytes)
}

fn private_mode(metadata: &std::fs::Metadata) -> Result<(), String> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if metadata.permissions().mode() & 0o077 != 0 {
            return Err("Pilot files must be private (0600 files, 0700 directories).".into());
        }
    }
    #[cfg(not(unix))]
    let _ = metadata;
    Ok(())
}

#[cfg(test)]
mod tests;
