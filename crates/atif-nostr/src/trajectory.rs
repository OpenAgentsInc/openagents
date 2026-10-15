//! Closed manifests, bounded chunks, and exact-byte/rendered verification.
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::sync::OnceLock;

use nostr::contracts::{self, SchemaClosure, digest_bytes, jcs, parse_strict_bounded};
use serde_json::{Value, json};

use crate::{CHUNK_SCHEMA, MANIFEST_SCHEMA, MAX_BYTES, MAX_CHUNKS, MAX_TEXT, Result};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Form {
    Log,
    Document,
}
impl Form {
    pub fn word(self) -> &'static str {
        match self {
            Self::Log => "log",
            Self::Document => "document",
        }
    }
    fn media(self) -> &'static str {
        match self {
            Self::Log => "application/jsonl",
            Self::Document => "application/json",
        }
    }
}

/// Optional source relationships. Every supplied object is schema-checked.
#[derive(Clone, Debug, Default)]
pub struct Options {
    pub derivation: Option<Value>,
    pub task: Option<Value>,
    pub run: Option<String>,
    pub selector: Option<Value>,
    pub parent: Option<Value>,
    pub children: Vec<Value>,
    pub previous: Option<Value>,
}

/// A closed manifest whose shape and semantic bounds have been checked.
#[derive(Clone, Debug)]
pub struct Manifest(Value);
impl Manifest {
    pub fn parse(value: Value) -> Result<Self> {
        schema(&value, false)?;
        if !atif::supported(string(&value, "schema_version")?) {
            return Err("unsupported_version: ATIF version".into());
        }
        id(string(&value, "trajectory_id")?)?;
        if let Some(session) = value["session_id"].as_str() {
            id(session)?;
        }
        let artifact = &value["artifact"];
        digest(string(artifact, "digest")?)?;
        hex(string(&value, "steps_digest")?)?;
        let form = string(&value, "form")?;
        let media = if form == "log" {
            "application/jsonl"
        } else {
            "application/json"
        };
        if artifact["schema"] != value["schema_version"]
            || artifact["media_type"] != media
            || artifact["size"]
                .as_u64()
                .is_none_or(|n| n == 0 || n > MAX_BYTES as u64)
        {
            return Err("manifest artifact does not match its form, version, or byte bound".into());
        }
        if value["coverage"] == "complete"
            && (value["state"] != "ended"
                || matches!(
                    value["derivation"]["kind"].as_str(),
                    Some("redacted" | "truncated")
                ))
        {
            return Err(
                "a changed or interrupted trajectory cannot claim complete coverage".into(),
            );
        }
        if let Some(source) = value["derivation"]["source"].as_str() {
            hex(source)?;
        }
        for field in ["task_id", "trajectory_id"] {
            for link in [&value["task"], &value["parent"], &value["previous"]] {
                if let Some(text) = link[field].as_str() {
                    id(text)?;
                }
            }
        }
        if let Some(run) = value["run"].as_str() {
            hex(run)?;
        }
        if let Some(previous) = value["previous"]["steps_digest"].as_str() {
            hex(previous)?;
        }
        for field in ["set", "bank"] {
            if let Some(pin) = value["selector"][field].as_str() {
                let (name, hash) = pin
                    .rsplit_once('@')
                    .ok_or("selector must pin name@digest")?;
                if name.is_empty() {
                    return Err("selector name is empty".into());
                }
                hex(hash.strip_prefix("sha256:").unwrap_or(hash))?;
            }
        }
        let chunks = array(&value, "chunks")?;
        let mut bytes = 0_u64;
        for (index, chunk) in chunks.iter().enumerate() {
            if chunk["index"].as_u64() != Some(index as u64) {
                return Err("chunk indexes are not ordered".into());
            }
            digest(string(chunk, "digest")?)?;
            if let Some(event) = chunk["event"].as_str() {
                hex(event)?;
            }
            bytes = bytes
                .checked_add(chunk["size"].as_u64().ok_or("chunk size")?)
                .ok_or("chunk size overflow")?;
        }
        if (!chunks.is_empty() && Some(bytes) != artifact["size"].as_u64())
            || (chunks.is_empty() && artifact["sources"].as_array().is_none_or(Vec::is_empty))
        {
            return Err("manifest chunks do not cover its artifact".into());
        }
        let mut children = BTreeSet::new();
        for child in array(&value, "children")? {
            id(string(child, "trajectory_id")?)?;
            if child["trajectory_id"] == value["trajectory_id"]
                || !children.insert(string(child, "trajectory_id")?)
            {
                return Err("duplicate or self child trajectory".into());
            }
            hex(string(child, "steps_digest")?)?;
            digest(string(child, "artifact")?)?;
            if child["step_id"]
                .as_u64()
                .is_none_or(|n| n > value["step_count"].as_u64().unwrap_or(0))
            {
                return Err("child step is outside the trajectory".into());
            }
            if let Some(event) = child["event"].as_str() {
                hex(event)?;
            }
        }
        Ok(Self(value))
    }
    pub fn body(&self) -> &Value {
        &self.0
    }
    pub fn trajectory_id(&self) -> &str {
        self.0["trajectory_id"].as_str().unwrap_or_default()
    }
    pub fn chunks(&self) -> &[Value] {
        self.0["chunks"]
            .as_array()
            .map(Vec::as_slice)
            .unwrap_or_default()
    }
    pub(crate) fn with_event_ids(&self, events: &[String]) -> Result<Self> {
        if events.len() != self.chunks().len() {
            return Err("chunk event count differs".into());
        }
        let mut value = self.0.clone();
        for (chunk, event) in value["chunks"]
            .as_array_mut()
            .ok_or("chunks")?
            .iter_mut()
            .zip(events)
        {
            chunk["event"] = json!(event);
        }
        Self::parse(value)
    }
}

#[derive(Clone, Debug)]
pub struct Package {
    pub manifest: Manifest,
    pub chunks: Vec<Value>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Verification {
    Verified,
    Partial,
    Unverified,
}
impl Verification {
    pub fn word(self) -> &'static str {
        match self {
            Self::Verified => "verified",
            Self::Partial => "partial",
            Self::Unverified => "unverified",
        }
    }
}
#[derive(Clone, Debug)]
pub struct Verified {
    pub manifest: Manifest,
    pub bytes: Vec<u8>,
    pub document: Value,
    pub status: Verification,
}

struct Rendered {
    document: Value,
    version: String,
    ended: bool,
    whole: bool,
    state_known: bool,
}
fn render(bytes: &[u8], form: Form) -> Result<Rendered> {
    if bytes.is_empty() || bytes.len() > MAX_BYTES {
        return Err("trajectory exceeds its byte bound or is empty".into());
    }
    let value = match form {
        Form::Log => {
            let log = atif::log::read_bytes(Path::new("shared.atif.jsonl"), bytes)
                .map_err(|e| e.to_string())?;
            Rendered {
                document: log.document(),
                version: log.schema_version.clone(),
                ended: log.ended(),
                whole: log.whole(),
                state_known: true,
            }
        }
        Form::Document => {
            let document = parse_strict_bounded(bytes, MAX_BYTES).map_err(|e| e.to_string())?;
            let ended = document.pointer("/extra/state").and_then(Value::as_str) == Some("ended");
            let version = string(&document, "schema_version")?.to_string();
            Rendered {
                state_known: document.pointer("/extra/state").is_some(),
                document,
                version,
                ended,
                whole: false,
            }
        }
    };
    if !atif::supported(&value.version) {
        return Err("unsupported_version: ATIF version".into());
    }
    if !atif::validate(&value.document).is_empty() {
        return Err("trajectory does not match its ATIF version".into());
    }
    id(string(&value.document, "trajectory_id")?)?;
    let steps = array(&value.document, "steps")?;
    for (index, step) in steps.iter().enumerate() {
        if step["step_id"].as_u64() != Some(index as u64 + 1) {
            return Err("ATIF steps are not ordered one-based ordinals".into());
        }
    }
    Ok(value)
}

pub fn prepare(bytes: &[u8], form: Form, options: Options) -> Result<Package> {
    let rendered = render(bytes, form)?;
    std::str::from_utf8(bytes).map_err(|_| "trajectory is not UTF-8")?;
    let artifact = digest_bytes(bytes);
    let mut texts = Vec::new();
    let mut offset = 0;
    while offset < bytes.len() {
        let text = std::str::from_utf8(&bytes[offset..]).map_err(|_| "UTF-8 boundary")?;
        let mut end = text.len().min(MAX_TEXT);
        while !text.is_char_boundary(end) {
            end -= 1;
        }
        // Keep records intact when one record fits in a chunk. A long record
        // alone may span chunks; never combine its tail with another record.
        if form == Form::Log {
            if let Some(newline) = text[..end].rfind('\n') {
                end = newline + 1;
            }
        }
        loop {
            let probe = json!({"v":CHUNK_SCHEMA,"requires":[],"trajectory_id":rendered.document["trajectory_id"],"artifact":artifact,"index":MAX_CHUNKS,"count":MAX_CHUNKS,"digest":digest_bytes(text[..end].as_bytes()),"text":&text[..end]});
            if jcs(&probe).map_err(|e| e.to_string())?.len() <= 60_000 {
                break;
            }
            end /= 2;
            while !text.is_char_boundary(end) {
                end -= 1;
            }
            if end == 0 {
                return Err("chunk cannot fit the encryption bound".into());
            }
            if form == Form::Log
                && let Some(newline) = text[..end].rfind('\n')
            {
                end = newline + 1;
            }
        }
        texts.push(text[..end].to_string());
        if texts.len() > MAX_CHUNKS {
            return Err("trajectory needs too many chunks; use continued segments".into());
        }
        offset += end;
    }
    let chunks: Vec<_> = texts.iter().enumerate().map(|(index,text)| json!({"v":CHUNK_SCHEMA,"requires":[],"trajectory_id":rendered.document["trajectory_id"],"artifact":artifact,"index":index,"count":texts.len(),"digest":digest_bytes(text.as_bytes()),"text":text})).collect();
    let partial = !rendered.whole
        || options
            .derivation
            .as_ref()
            .is_some_and(|v| matches!(v["kind"].as_str(), Some("redacted" | "truncated")));
    let manifest = Manifest::parse(json!({
        "v":MANIFEST_SCHEMA,"requires":[],"trajectory_id":rendered.document["trajectory_id"],
        "session_id":rendered.document.get("session_id").cloned().unwrap_or(Value::Null),
        "schema_version":rendered.version,"form":form.word(),
        "artifact":{"digest":artifact,"size":bytes.len(),"media_type":form.media(),"schema":rendered.version},
        "steps_digest":atif::digest(&rendered.document["steps"]),"step_count":array(&rendered.document,"steps")?.len(),
        "state":if rendered.ended {"ended"}else{"interrupted"},"coverage":if partial {"partial"}else{"complete"},
        "derivation":options.derivation,"task":options.task,"run":options.run,"selector":options.selector,
        "parent":options.parent,"children":options.children,"previous":options.previous,
        "chunks":chunks.iter().map(|v|json!({"index":v["index"],"digest":v["digest"],"size":v["text"].as_str().map(str::len)})).collect::<Vec<_>>()
    }))?;
    Ok(Package { manifest, chunks })
}

pub fn verify(manifest: &Manifest, chunks: &[Value]) -> Result<Verified> {
    let mut ordered = BTreeMap::new();
    for chunk in chunks {
        schema(chunk, true)?;
        let index = chunk["index"].as_u64().ok_or("chunk index")? as usize;
        let entry = manifest
            .chunks()
            .get(index)
            .ok_or("chunk index exceeds manifest")?;
        let text = string(chunk, "text")?;
        if text.len() > MAX_TEXT
            || jcs(chunk).map_err(|e| e.to_string())?.len() > 65_535
            || chunk["trajectory_id"] != manifest.0["trajectory_id"]
            || chunk["artifact"] != manifest.0["artifact"]["digest"]
            || chunk["count"].as_u64() != Some(manifest.chunks().len() as u64)
            || chunk["digest"] != entry["digest"]
            || chunk["digest"] != digest_bytes(text.as_bytes())
            || entry["size"].as_u64() != Some(text.len() as u64)
        {
            return Err("chunk does not match the manifest and exact text bytes".into());
        }
        if ordered.insert(index, text).is_some() {
            return Err("duplicate chunk index".into());
        }
    }
    if ordered.len() != manifest.chunks().len() || ordered.is_empty() {
        return Err("content_unavailable: missing trajectory chunks".into());
    }
    let bytes = ordered
        .values()
        .flat_map(|s| s.as_bytes().iter().copied())
        .collect::<Vec<_>>();
    verify_bytes(manifest, bytes)
}

pub fn verify_bytes(manifest: &Manifest, bytes: Vec<u8>) -> Result<Verified> {
    if bytes.len() > MAX_BYTES
        || manifest.0["artifact"]["size"].as_u64() != Some(bytes.len() as u64)
        || manifest.0["artifact"]["digest"] != digest_bytes(&bytes)
    {
        return Err("trajectory bytes differ from the manifest".into());
    }
    let form = if manifest.0["form"] == "log" {
        Form::Log
    } else {
        Form::Document
    };
    let rendered = render(&bytes, form)?;
    if manifest.0["trajectory_id"] != rendered.document["trajectory_id"]
        || manifest.0["session_id"]
            != rendered
                .document
                .get("session_id")
                .cloned()
                .unwrap_or(Value::Null)
        || manifest.0["schema_version"] != rendered.version
        || manifest.0["steps_digest"] != atif::digest(&rendered.document["steps"])
        || manifest.0["step_count"].as_u64()
            != Some(array(&rendered.document, "steps")?.len() as u64)
        || (rendered.state_known && (manifest.0["state"] == "ended") != rendered.ended)
        || (manifest.0["coverage"] == "complete" && !rendered.whole)
    {
        return Err("rendered ATIF fields differ from the manifest".into());
    }
    let status = if manifest.0["coverage"] == "partial" {
        Verification::Partial
    } else if rendered.state_known {
        Verification::Verified
    } else {
        Verification::Unverified
    };
    Ok(Verified {
        manifest: manifest.clone(),
        bytes,
        document: rendered.document,
        status,
    })
}

pub fn check_child(parent: &Verified, child: &Verified) -> Result<()> {
    let link = array(parent.manifest.body(), "children")?
        .iter()
        .find(|v| v["trajectory_id"] == child.manifest.0["trajectory_id"])
        .ok_or("child is an unconfirmed claim")?;
    let back = &child.manifest.0["parent"];
    if link["steps_digest"] != child.manifest.0["steps_digest"]
        || link["artifact"] != child.manifest.0["artifact"]["digest"]
        || back["trajectory_id"] != parent.manifest.0["trajectory_id"]
        || back["step_id"] != link["step_id"]
    {
        return Err("parent and child links disagree".into());
    }
    Ok(())
}
pub fn check_previous(current: &Verified, previous: &Verified) -> Result<()> {
    let link = &current.manifest.0["previous"];
    if link["trajectory_id"] != previous.manifest.0["trajectory_id"]
        || link["steps_digest"] != previous.manifest.0["steps_digest"]
        || current.manifest.0["session_id"] != previous.manifest.0["session_id"]
        || current.manifest.trajectory_id() == previous.manifest.trajectory_id()
    {
        return Err("continued trajectory link disagrees".into());
    }
    Ok(())
}

fn schema(value: &Value, chunk: bool) -> Result<()> {
    static SCHEMAS: OnceLock<Result<SchemaClosure>> = OnceLock::new();
    let closure = SCHEMAS
        .get_or_init(|| {
            let documents = [
                include_bytes!("../../../nips/openagents/schemas/atif-manifest.v1.json").as_slice(),
                include_bytes!("../../../nips/openagents/schemas/atif-chunk.v1.json").as_slice(),
            ]
            .into_iter()
            .map(|b| (digest_bytes(b), b.to_vec()))
            .collect();
            contracts::prepare_closure(&documents).map_err(|e| e.to_string())
        })
        .as_ref()
        .map_err(Clone::clone)?;
    if jcs(value).map_err(|e| e.to_string())?.len() > if chunk { 65_535 } else { 1_048_576 } {
        return Err("ATIF object exceeds its byte bound".into());
    }
    let bytes = if chunk {
        include_bytes!("../../../nips/openagents/schemas/atif-chunk.v1.json").as_slice()
    } else {
        include_bytes!("../../../nips/openagents/schemas/atif-manifest.v1.json").as_slice()
    };
    contracts::validate_instance(closure, &digest_bytes(bytes), value).map_err(|e| e.to_string())
}
pub(crate) fn hex(text: &str) -> Result<()> {
    if text.len() != 64
        || !text
            .bytes()
            .all(|b| b.is_ascii_digit() || matches!(b, b'a'..=b'f'))
    {
        return Err("expected a lowercase 64-hex identity".into());
    }
    Ok(())
}
fn digest(text: &str) -> Result<()> {
    hex(text
        .strip_prefix("sha256:")
        .ok_or("expected sha256: digest")?)
}
fn id(text: &str) -> Result<()> {
    if text.is_empty() || text.len() > 256 {
        Err("trajectory identity is empty or too long".into())
    } else {
        Ok(())
    }
}
fn string<'a>(value: &'a Value, key: &str) -> Result<&'a str> {
    value[key]
        .as_str()
        .ok_or_else(|| format!("missing text field {key}"))
}
fn array<'a>(value: &'a Value, key: &str) -> Result<&'a Vec<Value>> {
    value[key]
        .as_array()
        .ok_or_else(|| format!("missing array field {key}"))
}
