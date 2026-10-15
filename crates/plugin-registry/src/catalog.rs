//! The intrinsic REG contract is checked independently of a curator's schema.
use crate::{CATALOG_LIMIT, Result, SCHEMA_BYTES};
use nostr::contracts::{
    self, ArtifactRef, SchemaClosure, digest_bytes, jcs, parse_artifact, parse_schema_ref,
    parse_strict,
};
use nostr::domain::Event;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::sync::OnceLock;

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct Id(String);
impl Id {
    pub fn parse(text: &str) -> Result<Self> {
        let (key, slug) = text
            .split_once(':')
            .ok_or("registry/package ID needs publisher:slug")?;
        hex(key)?;
        if slug.is_empty()
            || slug.len() > 64
            || !slug.as_bytes()[0].is_ascii_lowercase() && !slug.as_bytes()[0].is_ascii_digit()
            || !slug
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || matches!(b, b'_' | b'-'))
        {
            return Err("invalid registry/package slug".into());
        }
        Ok(Self(text.into()))
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
    pub fn publisher(&self) -> &str {
        self.0.split_once(':').map(|v| v.0).unwrap_or_default()
    }
    pub fn slug(&self) -> &str {
        self.0.split_once(':').map(|v| v.1).unwrap_or_default()
    }
    pub fn address(&self, kind: u16) -> String {
        format!("{kind}:{}", self.0)
    }
}
impl TryFrom<String> for Id {
    type Error = String;
    fn try_from(s: String) -> Result<Self> {
        Self::parse(&s)
    }
}
impl From<Id> for String {
    fn from(id: Id) -> String {
        id.0
    }
}

#[derive(Clone, Debug)]
pub struct Catalog {
    body: Value,
    id: Id,
}
impl Catalog {
    pub fn parse(body: Value) -> Result<Self> {
        if jcs(&body).map_err(|e| e.to_string())?.len() > CATALOG_LIMIT {
            return Err("catalog exceeds its byte bound".into());
        }
        static CLOSURE: OnceLock<Result<SchemaClosure>> = OnceLock::new();
        let closure = CLOSURE
            .get_or_init(|| {
                contracts::prepare_closure(&BTreeMap::from([(
                    digest_bytes(SCHEMA_BYTES),
                    SCHEMA_BYTES.to_vec(),
                )]))
                .map_err(|e| e.to_string())
            })
            .as_ref()
            .map_err(Clone::clone)?;
        contracts::validate_instance(closure, &digest_bytes(SCHEMA_BYTES), &body)
            .map_err(|e| e.to_string())?;
        let id = Id::parse(text(&body, "registry")?)?;
        if number(&body, "valid_until")? <= number(&body, "as_of")? {
            return Err("catalog validity does not follow its observation time".into());
        }
        let mut previous = None;
        for entry in body["entries"].as_array().ok_or("entries")? {
            let package = Id::parse(text(entry, "package")?)?;
            if previous
                .as_ref()
                .is_some_and(|s: &String| s.as_str() >= package.as_str())
            {
                return Err("catalog entries must be unique and sorted by package ID".into());
            }
            previous = Some(package.as_str().to_string());
            event_ref(&entry["release"], 3184, Some(package.publisher()))?;
            let manifest = parse_artifact(&entry["manifest"]).map_err(|e| e.to_string())?;
            if manifest.schema.as_deref() != Some("openagents.package.v1")
                || manifest.media_type != "application/json"
            {
                return Err("member manifest must name openagents.package.v1 JSON".into());
            }
            if let Some(configuration) = entry.get("configuration") {
                parse_schema_ref(configuration).map_err(|e| e.to_string())?;
            }
            if let Some(evaluations) = entry["evaluation"].as_array() {
                for evaluation in evaluations {
                    if evaluation.get("id").is_some() {
                        event_ref(evaluation, 3189, None)?;
                    } else {
                        let reference = parse_artifact(evaluation).map_err(|e| e.to_string())?;
                        if reference.schema.as_deref() != Some("openagents.eval-report.v1") {
                            return Err("evaluation reference has another schema".into());
                        }
                    }
                }
            }
        }
        Ok(Self { body, id })
    }
    pub fn body(&self) -> &Value {
        &self.body
    }
    pub fn id(&self) -> &Id {
        &self.id
    }
    pub fn revision(&self) -> u64 {
        self.body["revision"].as_u64().unwrap_or(0)
    }
    pub fn entries(&self) -> &[Value] {
        self.body["entries"]
            .as_array()
            .map(Vec::as_slice)
            .unwrap_or_default()
    }
    pub fn entry(&self, package: &Id) -> Option<&Value> {
        self.entries()
            .iter()
            .find(|v| v["package"] == package.as_str())
    }
    pub fn fresh(&self, created_at: u64, now: u64, max_age: u64, skew: u64) -> Result<()> {
        let as_of = number(&self.body, "as_of")?;
        if created_at > now.saturating_add(skew)
            || as_of > created_at.saturating_add(skew)
            || as_of > now.saturating_add(skew)
            || now > number(&self.body, "valid_until")?
            || now.saturating_sub(as_of) > max_age
        {
            return Err("registry catalog is stale or from the future".into());
        }
        Ok(())
    }
}

/// Verify the whole inert catalog package, without reading member packages.
pub fn package_shape(release: &Event, manifest_bytes: &[u8]) -> Result<nostr::ext::Manifest> {
    let body = nostr::ext::parse_record(release).map_err(|e| e.to_string())?;
    if release.kind != 3184 {
        return Err("catalog is not an EXT release".into());
    }
    contracts::check_artifact_bytes(
        &parse_artifact(&body["manifest"]).map_err(|e| e.to_string())?,
        manifest_bytes,
    )
    .map_err(|e| e.to_string())?;
    let manifest = parse_strict(manifest_bytes).map_err(|e| e.to_string())?;
    let parsed = nostr::ext::parse_manifest(&manifest).map_err(|e| e.to_string())?;
    let id = Id::parse(&parsed.package)?;
    if body["package"] != id.as_str()
        || release.pubkey != id.publisher()
        || !parsed.dependencies.is_empty()
    {
        return Err("catalog package has another owner or executable dependencies".into());
    }
    if body["version"] != parsed.version {
        return Err("catalog version differs from release".into());
    }
    if !parsed
        .files
        .iter()
        .any(|f| f.path == "registry.json" && f.media_type == "application/json")
    {
        return Err("catalog package has no registry.json".into());
    }
    let schema_file = parsed
        .files
        .iter()
        .find(|f| f.path == "registry.schema.json" && f.media_type == "application/schema+json")
        .ok_or("catalog package has no registry.schema.json")?;
    let components = manifest["components"]
        .as_array()
        .ok_or("catalog components")?;
    if components
        .iter()
        .any(|c| !matches!(c["kind"].as_str(), Some("schema" | "guidance")))
        || !components.iter().any(|c| {
            c["slug"] == "catalog"
                && c["kind"] == "schema"
                && c["definition"]["digest"] == schema_file.digest
        })
    {
        return Err("registry catalog must contain an inert catalog schema component".into());
    }
    Ok(parsed)
}

/// Verify signed manifest bytes and all inert catalog files.
pub fn from_package(
    release: &Event,
    manifest_bytes: &[u8],
    files: &BTreeMap<String, Vec<u8>>,
    limit: u64,
) -> Result<Catalog> {
    let parsed = package_shape(release, manifest_bytes)?;
    let id = Id::parse(&parsed.package)?;
    let catalog_file = parsed
        .files
        .iter()
        .find(|f| f.path == "registry.json")
        .ok_or("catalog file")?;
    let schema_file = parsed
        .files
        .iter()
        .find(|f| f.path == "registry.schema.json")
        .ok_or("catalog schema")?;
    nostr::ext::verify_closure(&nostr::ext::Closure {
        manifest: &parsed,
        files,
        staged: &parsed
            .files
            .iter()
            .map(|f| f.path.clone())
            .collect::<Vec<_>>(),
        dependencies: &BTreeMap::from([(release.id.clone(), parsed.clone())]),
        root: &release.id,
        byte_limit: limit,
    })
    .map_err(|e| e.to_string())?;
    let catalog_bytes = files
        .get(&catalog_file.digest)
        .ok_or("catalog bytes unavailable")?;
    let catalog = Catalog::parse(parse_strict(catalog_bytes).map_err(|e| e.to_string())?)?;
    if catalog.id() != &id || parsed.version != format!("registry-{}", catalog.revision()) {
        return Err("catalog identity or revision differs from its package".into());
    }
    let schema_documents = parsed
        .files
        .iter()
        .filter(|f| f.media_type == "application/schema+json")
        .map(|f| {
            Ok((
                f.digest.clone(),
                files
                    .get(&f.digest)
                    .ok_or("schema file unavailable")?
                    .clone(),
            ))
        })
        .collect::<Result<BTreeMap<_, _>>>()?;
    let closure = contracts::prepare_closure(&schema_documents).map_err(|e| e.to_string())?;
    contracts::validate_instance(&closure, &schema_file.digest, catalog.body())
        .map_err(|e| e.to_string())?;
    Ok(catalog)
}

pub fn event_ref(value: &Value, kind: u16, publisher: Option<&str>) -> Result<String> {
    let object = value
        .as_object()
        .ok_or("event reference must be an object")?;
    if object
        .keys()
        .any(|k| !matches!(k.as_str(), "id" | "pubkey" | "kind" | "coordinate"))
    {
        return Err("unsupported event reference field".into());
    }
    let id = text(value, "id")?;
    hex(id)?;
    hex(text(value, "pubkey")?)?;
    if value["kind"].as_u64() != Some(u64::from(kind))
        || publisher.is_some_and(|p| value["pubkey"] != p)
    {
        return Err("event reference has another kind or publisher".into());
    }
    if let Some(coordinate) = value.get("coordinate") {
        if coordinate.as_str().is_none_or(str::is_empty) {
            return Err("event coordinate is not text".into());
        }
    }
    Ok(id.into())
}
pub fn matches_event(
    value: &Value,
    event: &Event,
    kind: u16,
    publisher: Option<&str>,
) -> Result<()> {
    let id = event_ref(value, kind, publisher)?;
    event.validate_crypto().map_err(|e| e.to_string())?;
    if id != event.id || value["pubkey"] != event.pubkey || event.kind != kind {
        return Err("event differs from its exact reference".into());
    }
    Ok(())
}
pub fn artifact_identity(left: &ArtifactRef, right: &ArtifactRef) -> bool {
    left.digest == right.digest
        && left.size == right.size
        && left.media_type == right.media_type
        && left.schema == right.schema
}
pub fn artifact(bytes: &[u8], media: &str, schema: Option<&str>) -> Value {
    let mut value = json!({"digest":digest_bytes(bytes),"size":bytes.len(),"media_type":media});
    if let Some(schema) = schema {
        value["schema"] = json!(schema);
    }
    value
}
pub fn reference(event: &Event) -> Value {
    json!({"id":event.id,"pubkey":event.pubkey,"kind":event.kind})
}
pub(crate) fn number(v: &Value, key: &str) -> Result<u64> {
    v[key]
        .as_u64()
        .filter(|n| *n <= 9_007_199_254_740_991)
        .ok_or_else(|| format!("invalid integer field {key}"))
}
pub(crate) fn text<'a>(v: &'a Value, key: &str) -> Result<&'a str> {
    v[key]
        .as_str()
        .ok_or_else(|| format!("missing text field {key}"))
}
pub(crate) fn hex(text: &str) -> Result<()> {
    if text.len() != 64
        || !text
            .bytes()
            .all(|b| b.is_ascii_digit() || matches!(b, b'a'..=b'f'))
    {
        Err("expected a lowercase 64-hex identity".into())
    } else {
        Ok(())
    }
}
