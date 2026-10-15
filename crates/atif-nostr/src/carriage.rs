//! Public declarations and recipient-specific encrypted private artifacts.
use crate::{CHUNK_SCHEMA, MANIFEST_SCHEMA, Manifest, Package, Result, Verified, verify};
use nostr::contracts::{digest_bytes, jcs, parse_strict_bounded};
use nostr::domain::{Event, RelaySigner, Tag};
use nostr::private_artifact;
use secp256k1::{SecretKey, XOnlyPublicKey};
use serde_json::{Value, json};
use std::str::FromStr;

pub const MANIFEST_KIND: u16 = 3_198;
pub const CHUNK_KIND: u16 = 3_199;
const MANIFEST_MARKER: &str = "oa:atif:trajectory:v1";
const CHUNK_MARKER: &str = "oa:atif:chunk:v1";

/// Chunks are published before the manifest. Persist this exact bundle to
/// retry publication without changing its event identities.
#[derive(Clone, Debug)]
pub struct Bundle {
    pub manifest: Event,
    pub chunks: Vec<Event>,
}

pub fn public(package: &Package, signer: &RelaySigner, now: u64, consent: bool) -> Result<Bundle> {
    if !consent {
        return Err("public trajectory sharing requires explicit consent".into());
    }
    check_package(package)?;
    let chunks = package
        .chunks
        .iter()
        .map(|chunk| {
            let content = canonical(chunk)?;
            Ok(signer.sign(now, CHUNK_KIND, tags(chunk, true)?, content))
        })
        .collect::<Result<Vec<_>>>()?;
    let manifest = package
        .manifest
        .with_event_ids(&chunks.iter().map(|e| e.id.clone()).collect::<Vec<_>>())?;
    let event = signer.sign(
        now,
        MANIFEST_KIND,
        tags(manifest.body(), false)?,
        canonical(manifest.body())?,
    );
    // Validate every event before any network operation can publish a prefix.
    validate_bundle(
        &Bundle {
            manifest: event.clone(),
            chunks: chunks.clone(),
        },
        None,
    )?;
    Ok(Bundle {
        manifest: event,
        chunks,
    })
}

pub fn private(
    package: &Package,
    secret: &SecretKey,
    recipient: &str,
    now: u64,
    retain_until: u64,
) -> Result<Bundle> {
    check_package(package)?;
    if retain_until <= now {
        return Err("private trajectory retention must end after publication".into());
    }
    let recipient = XOnlyPublicKey::from_str(recipient).map_err(|_| "invalid recipient key")?;
    let mailbox: String = secp256k1::rand::random::<[u8; 32]>()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    let seal = |value: &Value, schema: &str| -> Result<Event> {
        let bytes = jcs(value).map_err(|e| e.to_string())?;
        let envelope = json!({"v":"openagents.artifact-envelope.v1","requires":[],"artifact":{"digest":digest_bytes(&bytes),"size":bytes.len(),"media_type":"application/json","schema":schema},"inline":value,"issued_at":now,"retain_until":retain_until});
        private_artifact::seal(
            &envelope,
            secret,
            &recipient,
            &mailbox,
            now,
            secp256k1::rand::random(),
        )
        .map_err(|e| e.to_string())
    };
    let chunks = package
        .chunks
        .iter()
        .map(|v| seal(v, CHUNK_SCHEMA))
        .collect::<Result<Vec<_>>>()?;
    let manifest = package
        .manifest
        .with_event_ids(&chunks.iter().map(|e| e.id.clone()).collect::<Vec<_>>())?;
    let event = seal(manifest.body(), MANIFEST_SCHEMA)?;
    validate_bundle(
        &Bundle {
            manifest: event.clone(),
            chunks: chunks.clone(),
        },
        Some(secret),
    )?;
    Ok(Bundle {
        manifest: event,
        chunks,
    })
}

pub fn manifest(event: &Event, secret: Option<&SecretKey>) -> Result<Manifest> {
    let body = open(event, false, secret)?;
    Manifest::parse(body)
}

pub fn receive(event: &Event, chunks: &[Event], secret: Option<&SecretKey>) -> Result<Verified> {
    let manifest = manifest(event, secret)?;
    let mut bodies = Vec::new();
    for chunk in chunks {
        if chunk.pubkey != event.pubkey
            || (event.kind == MANIFEST_KIND) != (chunk.kind == CHUNK_KIND)
        {
            return Err("trajectory chunk has another author or disclosure scope".into());
        }
        if event.kind == 3_188
            && (tag(event, "p") != tag(chunk, "p") || tag(event, "h") != tag(chunk, "h"))
        {
            return Err("private trajectory chunk has another recipient or mailbox".into());
        }
        let body = open(chunk, true, secret)?;
        let index = body["index"].as_u64().ok_or("chunk index")? as usize;
        if let Some(expected) = manifest
            .chunks()
            .get(index)
            .and_then(|v| v["event"].as_str())
            && expected != chunk.id
        {
            return Err("chunk event differs from its exact manifest pin".into());
        }
        bodies.push(body);
    }
    verify(&manifest, &bodies)
}

fn open(event: &Event, chunk: bool, secret: Option<&SecretKey>) -> Result<Value> {
    event.validate_crypto().map_err(|e| e.to_string())?;
    let expected = if chunk { CHUNK_SCHEMA } else { MANIFEST_SCHEMA };
    if event.kind == 3_188 {
        let opened = private_artifact::open(
            event,
            secret.ok_or("private trajectory requires its reader key")?,
        )
        .map_err(|e| e.to_string())?;
        if opened.artifact().schema.as_deref() != Some(expected) {
            return Err("private artifact is not the expected ATIF object".into());
        }
        return parse_strict_bounded(
            opened.inline_bytes().ok_or("ATIF object is unavailable")?,
            1_048_576,
        )
        .map_err(|e| e.to_string());
    }
    if event.kind != if chunk { CHUNK_KIND } else { MANIFEST_KIND } {
        return Err("event is not an ATIF declaration or chunk".into());
    }
    let body =
        parse_strict_bounded(event.content.as_bytes(), 1_048_576).map_err(|e| e.to_string())?;
    let expected_tags = tags(&body, chunk)?;
    let mut actual: Vec<_> = event
        .tags
        .iter()
        .filter(|t| t.name() != Some("alt"))
        .map(|t| t.0.clone())
        .collect();
    let mut expected_tags = expected_tags.into_iter().map(|t| t.0).collect::<Vec<_>>();
    actual.sort();
    expected_tags.sort();
    if actual != expected_tags
        || event
            .tags
            .iter()
            .filter(|t| t.name() == Some("alt"))
            .count()
            > 1
        || event
            .tags
            .iter()
            .any(|t| t.name() == Some("alt") && t.0.len() != 2)
    {
        return Err("ATIF declaration tags do not match its body".into());
    }
    Ok(body)
}
fn tags(value: &Value, chunk: bool) -> Result<Vec<Tag>> {
    let digest = if chunk {
        &value["digest"]
    } else {
        &value["artifact"]["digest"]
    };
    let hash = digest
        .as_str()
        .and_then(|s| s.strip_prefix("sha256:"))
        .ok_or("ATIF artifact digest is missing")?;
    crate::trajectory::hex(hash)?;
    let mut tags = vec![
        Tag::new(vec![
            "t".into(),
            if chunk { CHUNK_MARKER } else { MANIFEST_MARKER }.into(),
        ]),
        Tag::new(vec!["x".into(), hash.into()]),
    ];
    if !chunk {
        for child in value["children"].as_array().ok_or("manifest children")? {
            if let Some(event) = child["event"].as_str() {
                tags.push(Tag::new(vec!["e".into(), event.into()]));
            }
        }
    }
    Ok(tags)
}
fn canonical(value: &Value) -> Result<String> {
    String::from_utf8(jcs(value).map_err(|e| e.to_string())?).map_err(|e| e.to_string())
}
fn tag<'a>(event: &'a Event, name: &str) -> Option<&'a str> {
    event
        .tags
        .iter()
        .find(|t| t.name() == Some(name))
        .and_then(Tag::value)
}

fn check_package(package: &Package) -> Result<()> {
    if package.manifest.chunks().is_empty() {
        if !package.chunks.is_empty() {
            return Err("source-only manifest has unexpected chunks".into());
        }
        Ok(())
    } else {
        verify(&package.manifest, &package.chunks).map(|_| ())
    }
}
/// Validate a signed publication before it sends any events. Source-only
/// declarations make claims; readers verify the separately fetched bytes.
pub fn validate_bundle(bundle: &Bundle, secret: Option<&SecretKey>) -> Result<()> {
    let manifest = manifest(&bundle.manifest, secret)?;
    for event in bundle
        .chunks
        .iter()
        .chain(std::iter::once(&bundle.manifest))
    {
        if serde_json::to_vec(&json!(["EVENT", event]))
            .map_err(|e| e.to_string())?
            .len()
            > 1_048_576
        {
            return Err("trajectory event exceeds the relay byte bound".into());
        }
    }
    if manifest.chunks().is_empty() {
        if !bundle.chunks.is_empty() {
            return Err("source-only declaration has unexpected chunks".into());
        }
        Ok(())
    } else {
        receive(&bundle.manifest, &bundle.chunks, secret).map(|_| ())
    }
}
