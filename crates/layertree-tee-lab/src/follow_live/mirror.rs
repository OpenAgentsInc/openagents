//! Directory/HTTPS and signed public NIP-01 mirror notes. No private app-data kinds.
use super::{
    chain::{client, endpoint, read_response},
    unhex,
};
use crate::{
    follow::{RecordSource, record_name, unroll_name},
    unroll::UnrollPackage,
};
use bitcoin::OutPoint;
use nostr::domain::Event;
use serde::Deserialize;
use serde_json::json;
use std::{
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

pub const MIRROR_LIMIT: u64 = 400_000;
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Relay {
    /// The existing signet controller uses `inline-file-v1`; absent means notes.
    #[serde(default)]
    pub inline_file: bool,
    pub tree_id: Option<String>,
    pub auth_env: Option<String>,
    pub url: String,
    pub publisher: String,
}
#[derive(Clone, Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Sources {
    #[serde(default)]
    pub directories: Vec<PathBuf>,
    #[serde(default)]
    pub https: Vec<String>,
    /// Public record bytes on explicitly selected RFC 1918 mirrors in the test VPC.
    #[serde(default)]
    pub private_http: Vec<String>,
    #[serde(default)]
    pub relays: Vec<Relay>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Body {
    schema: String,
    name: String,
    bytes: String,
}

pub fn decode_note(event: &Event, name: &str, publisher: &str) -> Result<Vec<u8>, String> {
    event
        .validate_nip01_structure()
        .map_err(|_| "Invalid mirror event.")?;
    event
        .validate_crypto()
        .map_err(|_| "Invalid mirror signature.")?;
    if event.kind != 1
        || event.pubkey != publisher
        || event.tag_values("d").collect::<Vec<_>>() != vec![name]
        || event.tag_values("t").collect::<Vec<_>>() != vec!["layertree-mirror-v1"]
    {
        return Err("The mirror event names another file or publisher.".into());
    }
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| "Clock is before Unix time.")?
        .as_secs();
    if event.is_expired(now) || event.created_at > now.saturating_add(300) {
        return Err("The mirror event is expired or dated in the future.".into());
    }
    let value = nostr::contracts::parse_strict_bounded(
        event.content.as_bytes(),
        (MIRROR_LIMIT * 2 + 1024) as usize,
    )
    .map_err(|_| "Invalid mirror body.")?;
    let body: Body = serde_json::from_value(value).map_err(|_| "Invalid mirror body fields.")?;
    if body.schema != "layertree.mirror.v1" || body.name != name {
        return Err("The mirror body names another file.".into());
    }
    unhex(&body.bytes, MIRROR_LIMIT as usize)
}

pub fn check_relay_url(text: &str) -> Result<(), String> {
    let url = reqwest::Url::parse(text).map_err(|_| "Invalid relay address.")?;
    let local = matches!(
        url.host_str(),
        Some("localhost" | "127.0.0.1" | "[::1]" | "::1")
    );
    if !matches!(url.scheme(), "ws" | "wss")
        || url.scheme() == "ws" && !local
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Err(
            "Use a wss relay, or loopback ws, without URL credentials or query parameters.".into(),
        );
    }
    Ok(())
}
pub fn mirror_endpoint(text: &str, private: bool) -> Result<reqwest::Url, String> {
    if !private {
        return endpoint(text);
    }
    let url = reqwest::Url::parse(text).map_err(|_| "Invalid private mirror address.")?;
    if url.scheme() != "http"
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
        || !url
            .host_str()
            .and_then(|host| host.parse::<std::net::Ipv4Addr>().ok())
            .is_some_and(|host| host.is_private())
    {
        return Err(
            "Choose an RFC 1918 HTTP mirror without credentials or query parameters.".into(),
        );
    }
    Ok(url)
}
pub struct Mirrors {
    sources: Sources,
    client: reqwest::blocking::Client,
    runtime: tokio::runtime::Runtime,
    cache: PathBuf,
    pub cache_failed: bool,
    record_hint: Option<[u8; 32]>,
}
impl Mirrors {
    pub fn new(sources: Sources, cache: PathBuf) -> Result<Self, String> {
        if sources.directories.is_empty()
            && sources.https.is_empty()
            && sources.private_http.is_empty()
            && sources.relays.is_empty()
        {
            return Err("Choose at least one record source.".into());
        }
        if sources.directories.len()
            + sources.https.len()
            + sources.private_http.len()
            + sources.relays.len()
            > 8
        {
            return Err("Choose up to eight independent record sources.".into());
        }
        for url in &sources.https {
            endpoint(url)?;
        }
        for url in &sources.private_http {
            mirror_endpoint(url, true)?;
        }
        for relay in &sources.relays {
            check_relay_url(&relay.url)?;
            if relay.inline_file {
                let tree = relay
                    .tree_id
                    .as_deref()
                    .ok_or("Inline-file mirrors need the tree ID.")?;
                if unhex(tree, 32)?.len() != 32 {
                    return Err("Inline-file mirrors need a 32-byte tree ID.".into());
                }
            }
            if let Some(env) = &relay.auth_env {
                let secret =
                    std::env::var(env).map_err(|_| "Set the dedicated relay reader key.")?;
                nostr::domain::RelaySigner::from_secret_hex(&secret)
                    .map_err(|_| "Invalid relay reader key.")?;
            }
            unhex(&relay.publisher, 32)?;
            if relay.publisher.len() != 64 {
                return Err("A mirror publisher needs a 32-byte public key.".into());
            }
        }
        std::fs::create_dir_all(&cache)
            .map_err(|_| "Could not create the follower's record cache.")?;
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|_| "Relay runtime could not start.")?;
        Ok(Self {
            sources,
            client: client()?,
            runtime,
            cache,
            cache_failed: false,
            record_hint: None,
        })
    }
    fn cached(&self, name: &str) -> Option<Vec<u8>> {
        read_file(&self.cache.join(name))
    }
    fn save(&mut self, name: &str, bytes: &[u8]) -> Option<Vec<u8>> {
        if self.cached(name).as_deref() != Some(bytes)
            && super::atomic_file(&self.cache.join(name), bytes).is_err()
        {
            self.cache_failed = true;
            return None;
        }
        Some(bytes.to_vec())
    }
    fn get_matching(
        &mut self,
        name: &str,
        check: &mut dyn FnMut(&[u8]) -> bool,
    ) -> Option<Vec<u8>> {
        let mut invalid = None;
        if let Some(bytes) = self.cached(name) {
            if check(&bytes) {
                return Some(bytes);
            }
            invalid = Some(bytes);
        }
        for dir in &self.sources.directories {
            if let Some(bytes) = read_file(&dir.join(name)) {
                if check(&bytes) {
                    return self.save(name, &bytes);
                }
                if invalid.is_none() {
                    invalid = Some(bytes);
                }
            }
        }
        for (base, private) in self
            .sources
            .https
            .iter()
            .map(|base| (base, false))
            .chain(self.sources.private_http.iter().map(|base| (base, true)))
        {
            let Ok(mut url) = mirror_endpoint(base, private) else {
                continue;
            };
            let Ok(mut path) = url.path_segments_mut() else {
                continue;
            };
            path.pop_if_empty().push(name);
            drop(path);
            if let Ok(response) = self.client.get(url).send() {
                if let Ok(bytes) = read_response(response, MIRROR_LIMIT) {
                    if check(&bytes) {
                        return self.save(name, &bytes);
                    }
                    if invalid.is_none() {
                        invalid = Some(bytes);
                    }
                }
            }
        }
        for relay in self.sources.relays.clone() {
            if relay.inline_file {
                let result = self.runtime.block_on(super::inline_file::fetch(
                    &relay,
                    name,
                    self.record_hint,
                ));
                if let Ok(candidates) = result {
                    for bytes in candidates {
                        if check(&bytes) {
                            return self.save(name, &bytes);
                        }
                        if invalid.is_none() {
                            invalid = Some(bytes);
                        }
                    }
                }
                continue;
            }
            let result = self.runtime.block_on(async {
                let filter = json!({"kinds":[1], "authors":[relay.publisher], "#d":[name], "#t":["layertree-mirror-v1"], "limit":8});
                let mut invalid = None;
                for event in super::inline_file::query(&relay, filter).await? {
                    if let Ok(bytes) = decode_note(&event, name, &relay.publisher) {
                        if check(&bytes) { return Ok((Some(bytes), invalid)); }
                        if invalid.is_none() { invalid = Some(bytes); }
                    }
                }
                Ok::<_, String>((None, invalid))
            });
            if let Ok((valid, bad)) = result {
                if let Some(bytes) = valid {
                    return self.save(name, &bytes);
                }
                if invalid.is_none() {
                    invalid = bad;
                }
            }
        }
        invalid
    }
}
fn read_file(path: &std::path::Path) -> Option<Vec<u8>> {
    use std::io::Read;
    let file = std::fs::File::open(path).ok()?;
    let mut bytes = Vec::new();
    file.take(MIRROR_LIMIT + 1).read_to_end(&mut bytes).ok()?;
    (bytes.len() as u64 <= MIRROR_LIMIT).then_some(bytes)
}
impl RecordSource for Mirrors {
    fn record(&mut self, hash: &[u8; 32]) -> Option<Vec<u8>> {
        let bytes = self.get_matching(&record_name(hash), &mut |bytes| {
            crate::tree::tagged_hash(crate::rules::TAG_RECORD, &[bytes]) == *hash
        });
        if bytes.as_deref().is_some_and(|bytes| {
            crate::tree::tagged_hash(crate::rules::TAG_RECORD, &[bytes]) == *hash
        }) {
            self.record_hint = Some(*hash);
        }
        bytes
    }
    fn unroll(&mut self, outpoint: &OutPoint) -> Option<Vec<u8>> {
        self.get_matching(&unroll_name(outpoint), &mut |bytes| {
            UnrollPackage::decode(bytes).is_ok_and(|p| p.tree_outpoint == *outpoint)
        })
    }
    fn unroll_matching(
        &mut self,
        outpoint: &OutPoint,
        check: &mut dyn FnMut(&[u8]) -> bool,
    ) -> Option<Vec<u8>> {
        self.get_matching(&unroll_name(outpoint), check)
            .filter(|bytes| check(bytes))
    }
}
