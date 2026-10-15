//! Admitted mirrors and relays. Catalog-provided URLs never widen these sources.
use crate::catalog::{Id, hex};
use crate::{CATALOG_LIMIT, Result};
use futures_util::StreamExt;
use nostr::contracts::{digest_bytes, parse_strict};
use nostr::domain::Event;
use nostr_transport::Connection;
use secp256k1::SecretKey;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::io::Read;
use std::path::{Component, Path, PathBuf};
use std::time::{Duration, Instant};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum Source {
    Directory {
        path: PathBuf,
    },
    Https {
        base: String,
    },
    Github {
        owner: String,
        repository: String,
        reference: String,
        directory: String,
    },
    Nostr {
        relay: String,
        blobs: Vec<String>,
    },
}
impl Source {
    pub fn validate(&self) -> Result<()> {
        match self {
            Self::Directory { path } => {
                if !path.is_absolute() {
                    return Err("registry directory must be an absolute path".into());
                }
            }
            Self::Https { base } => {
                base_url(base)?;
            }
            Self::Github {
                owner,
                repository,
                reference,
                directory,
            } => {
                for name in [owner, repository] {
                    if name.is_empty()
                        || name.len() > 128
                        || !name
                            .bytes()
                            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-' | b'.'))
                        || matches!(name.as_str(), "." | "..")
                    {
                        return Err("invalid GitHub owner or repository".into());
                    }
                }
                if reference.is_empty()
                    || reference.len() > 256
                    || reference.chars().any(char::is_control)
                {
                    return Err("invalid GitHub ref".into());
                }
                if !directory.is_empty() {
                    relative(directory)?;
                }
            }
            Self::Nostr { relay, blobs } => {
                let url = reqwest::Url::parse(relay).map_err(|_| "invalid registry relay URL")?;
                if !url.username().is_empty()
                    || url.password().is_some()
                    || url.query().is_some()
                    || url.fragment().is_some()
                    || !(url.scheme() == "wss"
                        || url.scheme() == "ws"
                            && matches!(url.host_str(), Some("127.0.0.1" | "localhost" | "[::1]")))
                {
                    return Err("registry relay needs wss or a loopback ws URL".into());
                }
                if blobs.len() > 8 {
                    return Err("too many admitted blob sources".into());
                }
                for base in blobs {
                    base_url(base)?;
                }
            }
        }
        Ok(())
    }
    /// Pure URL construction is shared by the HTTP adapter and its fixtures.
    pub fn url(&self, path: &str) -> Result<reqwest::Url> {
        relative(path)?;
        match self {
            Self::Https { base } => join_url(base, path),
            Self::Github {
                owner,
                repository,
                reference,
                directory,
            } => {
                self.validate()?;
                let mut url =
                    reqwest::Url::parse("https://api.github.com/").map_err(|e| e.to_string())?;
                {
                    let mut segments = url.path_segments_mut().map_err(|_| "GitHub URL")?;
                    segments.extend(["repos", owner.as_str(), repository.as_str(), "contents"]);
                    if !directory.is_empty() {
                        segments.extend(directory.split('/'));
                    }
                    segments.extend(path.split('/'));
                }
                url.query_pairs_mut().append_pair("ref", reference);
                Ok(url)
            }
            _ => Err("source has no HTTP mirror URL".into()),
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Limits {
    pub requests: usize,
    pub bytes: u64,
    pub seconds: u64,
}
impl Default for Limits {
    fn default() -> Self {
        Self {
            requests: 128,
            bytes: 64 * 1024 * 1024,
            seconds: 60,
        }
    }
}
impl Limits {
    pub fn validate(&self) -> Result<()> {
        if !(1..=4096).contains(&self.requests)
            || !(1..=512 * 1024 * 1024).contains(&self.bytes)
            || !(1..=120).contains(&self.seconds)
        {
            return Err("registry fetch bounds are outside supported limits".into());
        }
        Ok(())
    }
}
/// One budget across source failures, aliases, manifests and selected closures.
/// Content-addressed bytes and event identities are cached only for this fetch.
pub struct Reader {
    sources: Vec<Source>,
    registry: Id,
    limits: Limits,
    deadline: Instant,
    requests: usize,
    bytes: u64,
    secret: SecretKey,
    pub(crate) client: reqwest::Client,
    events: std::collections::BTreeMap<String, Event>,
    artifacts: std::collections::BTreeMap<String, Vec<u8>>,
    pub errors: Vec<String>,
}
impl Reader {
    pub fn new(sources: Vec<Source>, registry: Id, limits: Limits) -> Result<Self> {
        limits.validate()?;
        if sources.is_empty() || sources.len() > 8 {
            return Err("enroll one to eight registry sources".into());
        }
        for source in &sources {
            source.validate()?;
        }
        let client = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .timeout(Duration::from_secs(limits.seconds))
            .user_agent("OpenAgents-REG/1")
            .build()
            .map_err(|e| e.to_string())?;
        Ok(Self {
            sources,
            registry,
            deadline: Instant::now() + Duration::from_secs(limits.seconds),
            limits,
            requests: 0,
            bytes: 0,
            secret: SecretKey::from_byte_array(secp256k1::rand::random())
                .map_err(|_| "could not create registry reader identity")?,
            client,
            events: Default::default(),
            artifacts: Default::default(),
            errors: Vec::new(),
        })
    }
    pub fn usage(&self) -> Value {
        json!({"requests":self.requests,"bytes":self.bytes})
    }
    pub fn artifacts(&self) -> &std::collections::BTreeMap<String, Vec<u8>> {
        &self.artifacts
    }
    pub fn events(&self) -> &std::collections::BTreeMap<String, Event> {
        &self.events
    }
    pub fn current_time(&self, initial: u64) -> u64 {
        initial.saturating_add(
            (self.deadline - Duration::from_secs(self.limits.seconds))
                .elapsed()
                .as_secs(),
        )
    }
    fn request(&mut self) -> Result<Duration> {
        if self.requests >= self.limits.requests {
            return Err("registry request budget exhausted".into());
        }
        self.requests += 1;
        self.deadline
            .checked_duration_since(Instant::now())
            .filter(|t| !t.is_zero())
            .ok_or("registry fetch deadline exceeded".into())
    }
    fn charge(&mut self, count: usize) -> Result<()> {
        self.bytes = self.bytes.saturating_add(count as u64);
        if self.bytes > self.limits.bytes {
            return Err("registry byte budget exhausted".into());
        }
        Ok(())
    }
    async fn bytes_at(&mut self, source: &Source, path: &str, limit: usize) -> Result<Vec<u8>> {
        relative(path)?;
        let remaining = self.request()?;
        let limit = limit.min(self.limits.bytes.saturating_sub(self.bytes) as usize);
        match source {
            Source::Directory { path: root } => {
                let root = root
                    .canonicalize()
                    .map_err(|_| "registry directory is unavailable")?;
                let path = root
                    .join(path)
                    .canonicalize()
                    .map_err(|_| "registry object is unavailable")?;
                if !path.starts_with(&root) {
                    return Err("registry file escapes its enrolled directory".into());
                }
                if !std::fs::metadata(&path)
                    .map_err(|_| "registry file metadata is unavailable")?
                    .is_file()
                {
                    return Err("registry object is not a regular file".into());
                }
                let mut bytes = Vec::new();
                std::fs::File::open(path)
                    .map_err(|_| "registry object is unavailable")?
                    .take(limit as u64 + 1)
                    .read_to_end(&mut bytes)
                    .map_err(|_| "registry object read failed")?;
                self.charge(bytes.len())?;
                if bytes.len() > limit {
                    return Err("registry object exceeds its byte bound".into());
                }
                Ok(bytes)
            }
            Source::Https { .. } | Source::Github { .. } => {
                let url = source.url(path)?;
                self.http(
                    url,
                    matches!(source, Source::Github { .. }),
                    remaining,
                    limit,
                )
                .await
            }
            _ => Err("relay source has no mirror path".into()),
        }
    }
    async fn http(
        &mut self,
        url: reqwest::Url,
        github: bool,
        timeout: Duration,
        limit: usize,
    ) -> Result<Vec<u8>> {
        let mut request = self.client.get(url).timeout(timeout);
        if github {
            request = request
                .header("Accept", "application/vnd.github.raw+json")
                .header("X-GitHub-Api-Version", "2026-03-10");
        }
        let response = request
            .send()
            .await
            .map_err(|_| "registry HTTPS source is unavailable")?;
        if !response.status().is_success() {
            return Err(format!(
                "registry HTTPS source returned {}",
                response.status().as_u16()
            ));
        }
        if response.content_length().is_some_and(|n| n > limit as u64) {
            return Err("registry object exceeds its byte bound".into());
        }
        let mut stream = response.bytes_stream();
        let mut bytes = Vec::new();
        while let Some(part) = tokio::time::timeout(timeout, stream.next())
            .await
            .map_err(|_| "registry HTTPS read deadline exceeded")?
        {
            let part = part.map_err(|_| "registry HTTPS transfer failed")?;
            self.charge(part.len())?;
            if bytes.len().saturating_add(part.len()) > limit {
                return Err("registry object exceeds its byte bound".into());
            }
            bytes.extend_from_slice(&part);
        }
        Ok(bytes)
    }
    async fn query(&mut self, relay: &str, filter: Value, limit: usize) -> Result<Vec<Event>> {
        let remaining = self.request()?;
        let mut socket = Connection::connect_open(relay, remaining)
            .await?
            .with_frame_budget(limit + 8);
        let mut received = socket.received_bytes();
        self.charge(received)?;
        socket
            .send(json!(["REQ", "registry", filter.clone()]))
            .await?;
        let mut events = Vec::new();
        let mut authenticating = None;
        loop {
            let frame = socket.next().await;
            let count = socket.received_bytes();
            self.charge(count.saturating_sub(received))?;
            received = count;
            let frame = frame?;
            if frame[0] == "AUTH" {
                let challenge = frame[1]
                    .as_str()
                    .filter(|s| !s.is_empty() && s.len() <= 4096)
                    .ok_or("invalid relay challenge")?;
                let signer = nostr::domain::RelaySigner::from_secret_hex(
                    &self.secret.display_secret().to_string(),
                )
                .map_err(|_| "reader identity unavailable")?;
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map_err(|_| "clock is before Unix time")?
                    .as_secs();
                let event = signer.sign(
                    now,
                    22242,
                    vec![
                        nostr::domain::Tag::new(vec!["relay".into(), relay.into()]),
                        nostr::domain::Tag::new(vec!["challenge".into(), challenge.into()]),
                    ],
                    String::new(),
                );
                authenticating = Some(event.id.clone());
                socket.send(json!(["AUTH", event])).await?;
            } else if frame[0] == "OK" && authenticating.as_deref().is_some_and(|id| frame[1] == id)
            {
                if frame[2] != true {
                    return Err("relay refused the registry reader identity".into());
                }
                authenticating = None;
                socket
                    .send(json!(["REQ", "registry", filter.clone()]))
                    .await?;
            } else if frame[0] == "EVENT" && frame[1] == "registry" {
                let event: Event = serde_json::from_value(frame[2].clone())
                    .map_err(|_| "malformed registry event")?;
                if events.iter().any(|e: &Event| e.id == event.id) {
                    continue;
                }
                events.push(event);
                if events.len() == limit {
                    break;
                }
            } else if frame[0] == "EOSE" && frame[1] == "registry" && authenticating.is_none() {
                break;
            } else if frame[0] == "CLOSED"
                && frame[1] == "registry"
                && !(authenticating.is_some()
                    && frame[2]
                        .as_str()
                        .is_some_and(|s| s.starts_with("auth-required:")))
            {
                return Err("registry subscription was refused".into());
            }
        }
        socket.send(json!(["CLOSE", "registry"])).await?;
        socket.close().await?;
        Ok(events)
    }
    pub async fn heads(&mut self, id: &Id, kind: u16) -> Result<Vec<Event>> {
        let mut result = Vec::new();
        for source in self.sources.clone() {
            let found=match &source{
                Source::Nostr{relay,..}=>self.query(relay,json!({"kinds":[kind],"authors":[id.publisher()],"#d":[id.slug()],"limit":8}),8).await,
                _=>{
                    let path=format!("heads/{kind}/{}/{}.json",id.publisher(),id.slug());
                    let mut values=Vec::new();
                    for path in std::iter::once(path).chain((kind==30184&&id==&self.registry).then(||"registry.json".to_string())){
                        match self.bytes_at(&source,&path,CATALOG_LIMIT).await.and_then(parse_event){Ok(event)=>values.push(event),Err(e)=>self.errors.push(e)}
                    }
                    Ok(values)
                }
            };
            match found {
                Ok(events) => result.extend(events),
                Err(e) => self.errors.push(e),
            }
        }
        if result.is_empty() {
            return Err("content_unavailable: registry/checkpoint head".into());
        }
        Ok(result)
    }
    pub async fn event(&mut self, id: &str) -> Result<Event> {
        hex(id)?;
        if let Some(event) = self.events.get(id) {
            return Ok(event.clone());
        }
        for source in self.sources.clone() {
            let found = match &source {
                Source::Nostr { relay, .. } => self
                    .query(relay, json!({"ids":[id],"limit":1}), 1)
                    .await
                    .and_then(|mut e| e.pop().ok_or("event unavailable".into())),
                _ => self
                    .bytes_at(&source, &format!("events/{id}.json"), CATALOG_LIMIT)
                    .await
                    .and_then(parse_event),
            };
            match found {
                Ok(event) if event.id == id && event.validate_crypto().is_ok() => {
                    self.events.insert(id.into(), event.clone());
                    return Ok(event);
                }
                Ok(_) => self.errors.push("source returned another event".into()),
                Err(e) => self.errors.push(e),
            }
        }
        Err("content_unavailable: exact registry event".into())
    }
    pub async fn artifact(&mut self, digest: &str, size: u64) -> Result<Vec<u8>> {
        let hash = digest.strip_prefix("sha256:").ok_or("artifact digest")?;
        hex(hash)?;
        if size > self.limits.bytes {
            return Err("artifact exceeds the registry byte budget".into());
        }
        if let Some(bytes) = self.artifacts.get(digest) {
            if bytes.len() as u64 == size {
                return Ok(bytes.clone());
            }
            return Err("artifact size conflict".into());
        }
        for source in self.sources.clone() {
            let found = match &source {
                Source::Nostr { blobs, .. } => {
                    let mut bytes = None;
                    for base in blobs {
                        let remaining = self.request()?;
                        match self
                            .http(join_url(base, hash)?, false, remaining, size as usize)
                            .await
                        {
                            Ok(value)
                                if value.len() as u64 == size && digest_bytes(&value) == digest =>
                            {
                                bytes = Some(value);
                                break;
                            }
                            Ok(_) => self
                                .errors
                                .push("blob differs from its artifact pin".into()),
                            Err(e) => self.errors.push(e),
                        }
                    }
                    bytes.ok_or("blob unavailable".into())
                }
                _ => {
                    self.bytes_at(&source, &format!("artifacts/sha256/{hash}"), size as usize)
                        .await
                }
            };
            match found {
                Ok(bytes) if bytes.len() as u64 == size && digest_bytes(&bytes) == digest => {
                    self.artifacts.insert(digest.into(), bytes.clone());
                    return Ok(bytes);
                }
                Ok(_) => self
                    .errors
                    .push("source bytes differ from their artifact pin".into()),
                Err(e) => self.errors.push(e),
            }
        }
        Err("content_unavailable: pinned registry artifact".into())
    }
}
fn parse_event(bytes: Vec<u8>) -> Result<Event> {
    let value = parse_strict(&bytes).map_err(|e| e.to_string())?;
    serde_json::from_value(value).map_err(|_| "malformed registry event".into())
}
fn base_url(value: &str) -> Result<reqwest::Url> {
    let url = reqwest::Url::parse(value).map_err(|_| "invalid registry source URL")?;
    if url.scheme() != "https"
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Err(
            "registry mirror requires an HTTPS URL without credentials, query, or fragment".into(),
        );
    }
    Ok(url)
}
fn join_url(base: &str, path: &str) -> Result<reqwest::Url> {
    relative(path)?;
    let mut url = base_url(base)?;
    {
        let mut segments = url.path_segments_mut().map_err(|_| "mirror URL path")?;
        segments.pop_if_empty();
        segments.extend(path.split('/'));
    }
    Ok(url)
}
pub(crate) fn relative(path: &str) -> Result<()> {
    if path.is_empty()
        || path.len() > 2048
        || path.contains('\\')
        || !Path::new(path)
            .components()
            .all(|c| matches!(c, Component::Normal(_)))
    {
        return Err("registry path must stay inside its enrolled source".into());
    }
    Ok(())
}
