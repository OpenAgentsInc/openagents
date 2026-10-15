//! Local enrollment and monotone observations; snapshots are accepted atomically.
use crate::Result;
use crate::catalog::{self, Catalog, Id};
use crate::publish::{atomic, newer, read};
use crate::source::{Limits, Reader, Source};
use nostr::contracts::{self, ArtifactRef, digest_bytes, parse_artifact, parse_strict_bounded};
use nostr::domain::Event;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

const RECORD_LIMIT: usize = 32 * 1024 * 1024;
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Policy {
    pub limits: Limits,
    pub max_age: u64,
    pub skew: u64,
    pub allow_cached_browse: bool,
    pub allow_pinned_use: bool,
    pub require_current_membership: bool,
}
impl Default for Policy {
    fn default() -> Self {
        Self {
            limits: Limits::default(),
            max_age: 86_400,
            skew: 300,
            allow_cached_browse: true,
            allow_pinned_use: false,
            require_current_membership: true,
        }
    }
}
impl Policy {
    pub fn validate(&self) -> Result<()> {
        self.limits.validate()?;
        if self.max_age == 0 || self.max_age > 365 * 86_400 || self.skew > 3600 {
            return Err("invalid registry freshness policy".into());
        }
        Ok(())
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Snapshot {
    pub listing: Option<Event>,
    pub release: Event,
    pub revision: u64,
    pub catalog_digest: String,
    pub checked_at: u64,
    pub sources: Vec<Source>,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Revocations {
    pub observed: Option<Event>,
    pub accepted: Option<Event>,
    pub events: BTreeMap<String, Event>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Record {
    pub v: String,
    pub id: Id,
    pub enabled: bool,
    pub sources: Vec<Source>,
    pub policy: Policy,
    pub pinned_release: Option<String>,
    pub observed: Option<Event>,
    pub snapshot: Option<Snapshot>,
    pub revocations: BTreeMap<String, Revocations>,
    pub last_error: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Selection {
    pub v: String,
    pub registry: Id,
    pub catalog_release: String,
    pub catalog_manifest: String,
    pub catalog_digest: String,
    pub catalog_revision: u64,
    pub package: Id,
    pub release: Event,
    pub dependencies: BTreeMap<String, Event>,
    pub manifest: Value,
    pub selected_at: u64,
    pub sources: Vec<Source>,
    pub usage: Value,
}
pub struct Store {
    root: PathBuf,
    #[cfg(test)]
    pub(crate) client: Option<reqwest::Client>,
}
struct Locked {
    _lock: std::fs::File,
    record: Record,
}
impl Store {
    pub fn open(root: PathBuf) -> Result<Self> {
        if !root.exists() {
            let mut builder = std::fs::DirBuilder::new();
            builder.recursive(true);
            #[cfg(unix)]
            {
                use std::os::unix::fs::DirBuilderExt;
                builder.mode(0o700);
            }
            builder.create(&root).map_err(|e| e.to_string())?;
        }
        if !root.is_dir() {
            return Err("registry state path is not a directory".into());
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if std::fs::metadata(&root)
                .map_err(|e| e.to_string())?
                .permissions()
                .mode()
                & 0o077
                != 0
            {
                return Err("registry state directory must be readable only by its owner".into());
            }
        }
        Ok(Self {
            root,
            #[cfg(test)]
            client: None,
        })
    }
    fn stem(id: &Id) -> String {
        digest_bytes(id.as_str().as_bytes())[7..].to_string()
    }
    fn record_path(&self, id: &Id) -> PathBuf {
        self.root.join(format!("registry-{}.json", Self::stem(id)))
    }
    fn lock_file(&self, id: &Id) -> Result<std::fs::File> {
        let file = std::fs::OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(self.root.join(format!("registry-{}.lock", Self::stem(id))))
            .map_err(|e| e.to_string())?;
        fs2::FileExt::try_lock_exclusive(&file)
            .map_err(|_| "another registry operation is in progress")?;
        Ok(file)
    }
    fn locked(&self, id: &Id) -> Result<Locked> {
        let guard = self.lock_file(id)?;
        Ok(Locked {
            _lock: guard,
            record: self.record(id)?,
        })
    }
    pub fn record(&self, id: &Id) -> Result<Record> {
        let value = parse_strict_bounded(&read(&self.record_path(id), RECORD_LIMIT)?, RECORD_LIMIT)
            .map_err(|e| e.to_string())?;
        let record: Record = serde_json::from_value(value)
            .map_err(|_| "registry state is unreadable; it was not reset")?;
        if record.v != "openagents.registry-enrollment.v1" || &record.id != id {
            return Err("registry state has another identity or version".into());
        }
        record.policy.validate()?;
        for source in &record.sources {
            source.validate()?;
        }
        Ok(record)
    }
    fn save(&self, record: &Record) -> Result<()> {
        let bytes = serde_json::to_vec(record).map_err(|e| e.to_string())?;
        if bytes.len() > RECORD_LIMIT {
            return Err("registry state exceeds its retention bound".into());
        }
        atomic(&self.record_path(&record.id), &bytes)
    }
    pub fn enroll(&self, id: Id, sources: Vec<Source>, policy: Policy) -> Result<()> {
        self.enroll_with_pin(id, sources, policy, None)
    }
    pub fn enroll_with_pin(
        &self,
        id: Id,
        sources: Vec<Source>,
        policy: Policy,
        pin: Option<String>,
    ) -> Result<()> {
        if let Some(value) = &pin {
            catalog::hex(value)?;
        }
        policy.validate()?;
        let _guard = self.lock_file(&id)?;
        if sources.is_empty() || sources.len() > 8 {
            return Err("enroll one to eight registry sources".into());
        }
        for source in &sources {
            source.validate()?;
        }
        let mut record = if self.record_path(&id).exists() {
            self.record(&id)?
        } else {
            Record {
                v: "openagents.registry-enrollment.v1".into(),
                id: id.clone(),
                enabled: true,
                sources: sources.clone(),
                policy: policy.clone(),
                pinned_release: None,
                observed: None,
                snapshot: None,
                revocations: BTreeMap::new(),
                last_error: None,
            }
        };
        // Re-enrollment/source changes never erase observed heads, revisions,
        // cached snapshots, or publisher revocations.
        record.enabled = true;
        record.sources = sources;
        record.policy = policy;
        record.pinned_release = pin;
        record.last_error = Some("registry enrollment changed; refresh it".into());
        self.save(&record)
    }
    pub fn remove(&self, id: &Id) -> Result<()> {
        let mut locked = self.locked(id)?;
        locked.record.enabled = false;
        self.save(&locked.record)
    }
    pub fn list(&self) -> Result<Vec<Record>> {
        let mut out = Vec::new();
        for entry in std::fs::read_dir(&self.root).map_err(|e| e.to_string())? {
            let path = entry.map_err(|e| e.to_string())?.path();
            let name = path
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or_default();
            if !name.starts_with("registry-") || !name.ends_with(".json") {
                continue;
            }
            let value = parse_strict_bounded(&read(&path, RECORD_LIMIT)?, RECORD_LIMIT)
                .map_err(|e| e.to_string())?;
            let id = Id::parse(catalog::text(&value, "id")?)?;
            out.push(self.record(&id)?);
        }
        out.sort_by(|a, b| a.id.cmp(&b.id));
        Ok(out)
    }
    /// Group equal pins while preserving every curator and every conflicting pin.
    pub fn compare(&self, now: u64) -> Result<Value> {
        let mut packages: BTreeMap<String, BTreeMap<String, Value>> = BTreeMap::new();
        let mut unavailable = Vec::new();
        for record in self.list()?.into_iter().filter(|r| r.enabled) {
            match self.browse(&record.id, now, false) {
                Ok(catalog) => {
                    for entry in catalog["entries"].as_array().ok_or("catalog entries")? {
                        let key = Id::parse(catalog::text(entry, "package")?)?;
                        let manifest =
                            parse_artifact(&entry["manifest"]).map_err(|e| e.to_string())?;
                        let identity = format!(
                            "{}:{}:{}:{}:{:?}",
                            entry["release"]["id"],
                            manifest.digest,
                            manifest.size,
                            manifest.media_type,
                            manifest.schema
                        );
                        let pin=packages.entry(key.as_str().into()).or_default().entry(identity).or_insert_with(||json!({"release":entry["release"],"manifest":entry["manifest"],"curators":[]}));
                        pin["curators"]
                            .as_array_mut()
                            .ok_or("curators")?
                            .push(json!(record.id.as_str()));
                    }
                }
                Err(error) => {
                    unavailable.push(json!({"registry":record.id.as_str(),"error":error}))
                }
            }
        }
        let packages=packages.into_iter().map(|(package,pins)|json!({"package":package,"conflict":pins.len()>1,"choices":pins.into_values().collect::<Vec<_>>()})).collect::<Vec<_>>();
        Ok(json!({"packages":packages,"unavailable":unavailable}))
    }
    fn cache_path(&self, digest: &str) -> Result<PathBuf> {
        let hash = digest.strip_prefix("sha256:").ok_or("artifact digest")?;
        catalog::hex(hash)?;
        Ok(self.root.join(format!("artifacts/sha256/{hash}")))
    }
    fn cached_bytes(&self, reference: &ArtifactRef) -> Result<Vec<u8>> {
        if reference.size > 512 * 1024 * 1024 {
            return Err("cached artifact exceeds its byte bound".into());
        }
        let bytes = read(
            &self.cache_path(&reference.digest)?,
            reference.size as usize,
        )?;
        contracts::check_artifact_bytes(reference, &bytes).map_err(|e| e.to_string())?;
        Ok(bytes)
    }
    fn retain(&self, reader: &Reader) -> Result<()> {
        for (digest, bytes) in reader.artifacts() {
            if digest_bytes(bytes) != *digest {
                return Err("cache artifact digest differs".into());
            }
            atomic(&self.cache_path(digest)?, bytes)?;
        }
        for (id, event) in reader.events() {
            atomic(
                &self.root.join(format!("events/{id}.json")),
                &serde_json::to_vec(event).map_err(|e| e.to_string())?,
            )?;
        }
        Ok(())
    }
    fn cached_catalog(&self, record: &Record) -> Result<Catalog> {
        let snapshot = record
            .snapshot
            .as_ref()
            .ok_or("registry has no accepted catalog")?;
        let release = nostr::ext::parse_record(&snapshot.release).map_err(|e| e.to_string())?;
        let reference = parse_artifact(&release["manifest"]).map_err(|e| e.to_string())?;
        let manifest_bytes = self.cached_bytes(&reference)?;
        let parsed = catalog::package_shape(&snapshot.release, &manifest_bytes)?;
        if let Some(listing) = &snapshot.listing {
            let body = nostr::ext::parse_record(listing).map_err(|e| e.to_string())?;
            if body["package"] != record.id.as_str()
                || listing.kind != 30184
                || body["state"] != "published"
            {
                return Err("cached listing has another identity or state".into());
            }
            catalog::matches_event(
                &body["release"],
                &snapshot.release,
                3184,
                Some(record.id.publisher()),
            )?;
        }
        let mut files = BTreeMap::new();
        for file in &parsed.files {
            let reference = ArtifactRef {
                digest: file.digest.clone(),
                size: file.size,
                media_type: file.media_type.clone(),
                schema: None,
                event: None,
                sources: Vec::new(),
            };
            files.insert(file.digest.clone(), self.cached_bytes(&reference)?);
        }
        let catalog = catalog::from_package(
            &snapshot.release,
            &manifest_bytes,
            &files,
            record.policy.limits.bytes,
        )?;
        if catalog.id() != &record.id
            || catalog.revision() != snapshot.revision
            || parsed
                .files
                .iter()
                .find(|f| f.path == "registry.json")
                .is_none_or(|f| f.digest != snapshot.catalog_digest)
        {
            return Err("cached catalog differs from its accepted identity".into());
        }
        Ok(catalog)
    }
    pub fn browse(&self, id: &Id, now: u64, cached: bool) -> Result<Value> {
        let record = self.record(id)?;
        if !record.enabled {
            return Err("registry is not enrolled; prior selections were kept".into());
        }
        let catalog = self.cached_catalog(&record)?;
        let snapshot = record.snapshot.as_ref().ok_or("registry snapshot")?;
        let current = record.last_error.is_none()
            && record.pinned_release.is_none()
            && snapshot.listing.as_ref().is_some_and(|listing| {
                record.observed.as_ref().is_some_and(|e| e.id == listing.id)
            })
            && catalog
                .fresh(
                    snapshot.release.created_at,
                    now,
                    record.policy.max_age,
                    record.policy.skew,
                )
                .is_ok()
            && checkpoint_current(&record, &record.id, &snapshot.release.id, now).is_ok();
        if !current && !(cached && record.policy.allow_cached_browse) {
            return Err("registry is stale or withdrawn; refresh it or explicitly browse the permitted cache".into());
        }
        Ok(
            json!({"registry":id.as_str(),"revision":catalog.revision(),"catalog_release":snapshot.release.id,"fresh":current,"frozen":record.pinned_release.is_some(),"cached":true,"error":record.last_error,"entries":catalog.entries(),"sources":record.sources,"verification_limits":"only observed signed heads; unavailable or withheld newer state cannot be disproved"}),
        )
    }
    pub async fn refresh(&self, id: &Id, now: u64) -> Result<Value> {
        let mut locked = self.locked(id)?;
        if !locked.record.enabled {
            return Err("registry is not enrolled".into());
        }
        let mut reader = Reader::new(
            locked.record.sources.clone(),
            id.clone(),
            locked.record.policy.limits.clone(),
        )?;
        #[cfg(test)]
        if let Some(client) = &self.client {
            reader.client = client.clone();
        }
        let result = self
            .load_catalog(&mut locked.record, &mut reader, now)
            .await;
        match result {
            Ok(catalog) => {
                locked.record.last_error = None;
                self.save(&locked.record)?;
                Ok(
                    json!({"registry":id.as_str(),"revision":catalog.revision(),"entries":catalog.entries().len(),"usage":reader.usage(),"source_errors":reader.errors}),
                )
            }
            Err(error) => {
                locked.record.last_error = Some(error.clone());
                self.save(&locked.record)?;
                Err(error)
            }
        }
    }
    async fn load_catalog(
        &self,
        record: &mut Record,
        reader: &mut Reader,
        initial: u64,
    ) -> Result<Catalog> {
        let (head, release) = if let Some(pin) = &record.pinned_release {
            (None, reader.event(pin).await?)
        } else {
            let heads = reader.heads(&record.id, 30184).await?;
            let mut head = None;
            for event in heads {
                let Ok(body) = nostr::ext::parse_record(&event) else {
                    continue;
                };
                if event.kind != 30184
                    || event.pubkey != record.id.publisher()
                    || body["package"] != record.id.as_str()
                    || event.created_at
                        > reader
                            .current_time(initial)
                            .saturating_add(record.policy.skew)
                {
                    continue;
                }
                if record
                    .observed
                    .as_ref()
                    .is_some_and(|old| newer(old, &event))
                {
                    continue;
                }
                if head.as_ref().is_none_or(|old| newer(&event, old)) {
                    head = Some(event);
                }
            }
            let head = head.ok_or("registry returned no admissible current listing")?;
            if record.observed.as_ref().is_none_or(|old| newer(&head, old)) {
                record.observed = Some(head.clone());
                self.save(record)?;
            }
            let body = nostr::ext::parse_record(&head).map_err(|e| e.to_string())?;
            if body["state"] == "hidden" {
                return Err("registry curator withdrew its current recommendation".into());
            }
            let release_id =
                catalog::event_ref(&body["release"], 3184, Some(record.id.publisher()))?;
            let release = reader.event(&release_id).await?;
            catalog::matches_event(
                &body["release"],
                &release,
                3184,
                Some(record.id.publisher()),
            )?;
            (Some(head), release)
        };
        let declaration = nostr::ext::parse_record(&release).map_err(|e| e.to_string())?;
        if declaration["package"] != record.id.as_str() {
            return Err("catalog release belongs to another registry".into());
        }
        let manifest_ref = parse_artifact(&declaration["manifest"]).map_err(|e| e.to_string())?;
        let manifest_bytes = reader
            .artifact(&manifest_ref.digest, manifest_ref.size)
            .await?;
        contracts::check_artifact_bytes(&manifest_ref, &manifest_bytes)
            .map_err(|e| e.to_string())?;
        let parsed = catalog::package_shape(&release, &manifest_bytes)?;
        let mut files = BTreeMap::new();
        for file in &parsed.files {
            files.insert(
                file.digest.clone(),
                reader.artifact(&file.digest, file.size).await?,
            );
        }
        let catalog = catalog::from_package(
            &release,
            &manifest_bytes,
            &files,
            record.policy.limits.bytes,
        )?;
        let catalog_digest = parsed
            .files
            .iter()
            .find(|f| f.path == "registry.json")
            .ok_or("catalog file")?
            .digest
            .clone();
        catalog.fresh(
            release.created_at,
            reader.current_time(initial),
            record.policy.max_age,
            record.policy.skew,
        )?;
        if let Some(old) = &record.snapshot {
            if catalog.revision() < old.revision {
                return Err("registry catalog revision rolled back".into());
            }
            if catalog.revision()==old.revision&&(release.id!=old.release.id||manifest_ref.digest!=parse_artifact(&nostr::ext::parse_record(&old.release).map_err(|e|e.to_string())?["manifest"]).map_err(|e|e.to_string())?.digest||catalog_digest!=old.catalog_digest){return Err("registry revision has conflicting signed content".into());}
        }
        let id = record.id.clone();
        self.revocations(record, reader, &id, &release.id, initial)
            .await?;
        self.retain(reader)?;
        record.snapshot = Some(Snapshot {
            listing: head,
            release,
            revision: catalog.revision(),
            catalog_digest,
            checked_at: reader.current_time(initial),
            sources: record.sources.clone(),
        });
        self.save(record)?;
        Ok(catalog)
    }
    async fn revocations(
        &self,
        record: &mut Record,
        reader: &mut Reader,
        package: &Id,
        release: &str,
        initial: u64,
    ) -> Result<()> {
        let heads = reader.heads(package, 30185).await?;
        let now = reader.current_time(initial);
        let known = record
            .revocations
            .entry(package.as_str().into())
            .or_default();
        let mut candidate = None;
        for event in heads {
            let Ok(body) = nostr::ext::parse_record(&event) else {
                continue;
            };
            if event.kind != 30185
                || event.pubkey != package.publisher()
                || body["package"] != package.as_str()
                || event.created_at > now.saturating_add(record.policy.skew)
            {
                continue;
            }
            if known
                .observed
                .as_ref()
                .is_some_and(|old| newer(old, &event))
            {
                continue;
            }
            if candidate.as_ref().is_none_or(|old| newer(&event, old)) {
                candidate = Some(event);
            }
        }
        let head = candidate.ok_or("publisher returned no admissible current checkpoint")?;
        if known.observed.as_ref().is_none_or(|old| newer(&head, old)) {
            known.observed = Some(head.clone());
            self.save(record)?;
        }
        let body = nostr::ext::parse_record(&head).map_err(|e| e.to_string())?;
        let revision = catalog::number(&body, "revision")?;
        if revision == 0
            || catalog::number(&body, "as_of")? > head.created_at.saturating_add(record.policy.skew)
            || now.saturating_sub(catalog::number(&body, "as_of")?) > record.policy.max_age
        {
            return Err("publisher checkpoint is stale or has invalid revision".into());
        }
        let previous = record
            .revocations
            .get(package.as_str())
            .and_then(|v| v.accepted.as_ref())
            .map(|e| nostr::ext::parse_record(e).map_err(|e| e.to_string()))
            .transpose()?;
        let prior_revision = previous
            .as_ref()
            .map(|b| catalog::number(b, "revision"))
            .transpose()?
            .unwrap_or(0);
        if revision < prior_revision {
            return Err("publisher checkpoint revision rolled back".into());
        }
        let raw = if body["revocations"].is_array() {
            body["revocations"].clone()
        } else {
            let reference = parse_artifact(&body["revocations"]).map_err(|e| e.to_string())?;
            contracts::parse_strict(&reader.artifact(&reference.digest, reference.size).await?)
                .map_err(|e| e.to_string())?
        };
        let ids = raw
            .as_array()
            .filter(|a| a.len() <= 4096)
            .ok_or("revocation list exceeds its supported bound")?
            .iter()
            .map(|v| {
                v.as_str()
                    .map(str::to_string)
                    .ok_or("revocation ID is not text".to_string())
            })
            .collect::<Result<Vec<_>>>()?;
        for id in &ids {
            catalog::hex(id)?;
        }
        if ids.windows(2).any(|p| p[0] >= p[1]) {
            return Err("revocation IDs must be sorted and unique".into());
        }
        if let Some(old) = &previous
            && revision == prior_revision
            && old["revocations"] != body["revocations"]
        {
            return Err("publisher checkpoint revision has conflicting contents".into());
        }
        for id in &ids {
            if record.revocations[package.as_str()].events.contains_key(id) {
                continue;
            }
            let event = reader.event(id).await?;
            let value = nostr::ext::parse_record(&event).map_err(|e| e.to_string())?;
            if event.kind != 3185
                || event.pubkey != package.publisher()
                || value["package"] != package.as_str()
                || event.created_at
                    > reader
                        .current_time(initial)
                        .saturating_add(record.policy.skew)
            {
                return Err("revocation belongs to another package, publisher, or time".into());
            }
            catalog::event_ref(&value["release"], 3184, Some(package.publisher()))?;
            record
                .revocations
                .get_mut(package.as_str())
                .ok_or("revocation state")?
                .events
                .insert(id.clone(), event);
            // Retain each verified revocation even if a later fetch fails.
            self.save(record)?;
        }
        let known = &record.revocations[package.as_str()];
        let listed = ids.iter().cloned().collect::<BTreeSet<_>>();
        if known.events.keys().any(|id| !listed.contains(id)) {
            return Err("publisher checkpoint omitted a known revocation".into());
        }
        let now = reader.current_time(initial);
        nostr::ext::fresh(&nostr::ext::Freshness {
            as_of: catalog::number(&body, "as_of")?,
            valid_until: catalog::number(&body, "valid_until")?,
            now,
            skew: record.policy.skew,
            present: true,
            empty_answer: false,
            strict: true,
            explicit_pin: false,
        })
        .map_err(|e| e.to_string())?;
        record
            .revocations
            .get_mut(package.as_str())
            .ok_or("revocation state")?
            .accepted = Some(head);
        self.save(record)?;
        checkpoint_current(record, package, release, now)
    }
    pub async fn select(&self, id: &Id, package: &Id, now: u64) -> Result<Selection> {
        let mut locked = self.locked(id)?;
        if !locked.record.enabled {
            return Err("registry is not enrolled".into());
        }
        let mut reader = Reader::new(
            locked.record.sources.clone(),
            id.clone(),
            locked.record.policy.limits.clone(),
        )?;
        #[cfg(test)]
        if let Some(client) = &self.client {
            reader.client = client.clone();
        }
        let result = self
            .select_inner(&mut locked.record, &mut reader, package, now)
            .await;
        match result {
            Ok(selection) => {
                locked.record.last_error = None;
                self.save(&locked.record)?;
                Ok(selection)
            }
            Err(error) => {
                locked.record.last_error = Some(error.clone());
                self.save(&locked.record)?;
                Err(error)
            }
        }
    }
    async fn select_inner(
        &self,
        record: &mut Record,
        reader: &mut Reader,
        package: &Id,
        initial: u64,
    ) -> Result<Selection> {
        if record.pinned_release.is_some()
            && (record.policy.require_current_membership || !record.policy.allow_pinned_use)
        {
            return Err("frozen registry selection requires an explicit pinned-use policy without current membership".into());
        }
        let catalog = self.load_catalog(record, reader, initial).await?;
        let entry = catalog
            .entry(package)
            .ok_or("registry does not currently recommend that package")?
            .clone();
        let root_id = catalog::event_ref(&entry["release"], 3184, Some(package.publisher()))?;
        let mut pending = vec![root_id.clone()];
        let mut events = BTreeMap::new();
        let mut manifests = BTreeMap::new();
        let mut bytes = BTreeMap::new();
        let mut raw_root = None;
        while let Some(id) = pending.pop() {
            if manifests.contains_key(&id) {
                continue;
            }
            if manifests.len() >= 256 {
                return Err("selected package dependency closure exceeds its bound".into());
            }
            let event = reader.event(&id).await?;
            let body = nostr::ext::parse_record(&event).map_err(|e| e.to_string())?;
            if event.kind != 3184 {
                return Err("dependency is not an immutable EXT release".into());
            }
            let owner = Id::parse(catalog::text(&body, "package")?)?;
            let reference = parse_artifact(&body["manifest"]).map_err(|e| e.to_string())?;
            if id == root_id {
                catalog::matches_event(&entry["release"], &event, 3184, Some(package.publisher()))?;
                if body["package"] != package.as_str()
                    || !catalog::artifact_identity(
                        &reference,
                        &parse_artifact(&entry["manifest"]).map_err(|e| e.to_string())?,
                    )
                {
                    return Err("member release differs from the catalog pin".into());
                }
            }
            let value =
                contracts::parse_strict(&reader.artifact(&reference.digest, reference.size).await?)
                    .map_err(|e| e.to_string())?;
            let manifest = nostr::ext::parse_manifest(&value).map_err(|e| e.to_string())?;
            if manifest.package != owner.as_str() || body["version"] != manifest.version {
                return Err("member manifest differs from its publisher release".into());
            }
            self.revocations(record, reader, &owner, &id, initial)
                .await?;
            for file in &manifest.files {
                bytes.insert(
                    file.digest.clone(),
                    reader.artifact(&file.digest, file.size).await?,
                );
            }
            pending.extend(manifest.dependencies.iter().cloned());
            if id == root_id {
                raw_root = Some(value);
            }
            manifests.insert(id.clone(), manifest);
            events.insert(id, event);
        }

        // Verify every dependency's definition closure and reject cycles; a
        // catalog recommendation cannot substitute for missing executable bytes.
        for (id, manifest) in &manifests {
            nostr::ext::verify_closure(&nostr::ext::Closure {
                manifest,
                files: &bytes,
                staged: &manifest
                    .files
                    .iter()
                    .map(|f| f.path.clone())
                    .collect::<Vec<_>>(),
                dependencies: &manifests,
                root: id,
                byte_limit: record.policy.limits.bytes,
            })
            .map_err(|e| e.to_string())?;
        }
        if let Some(configuration) = entry.get("configuration") {
            let reference = contracts::parse_schema_ref(configuration)
                .map_err(|e| e.to_string())?
                .0;
            let allowed = manifests.values().any(|m| {
                m.files.iter().any(|f| {
                    f.digest == reference.digest
                        && f.size == reference.size
                        && f.media_type == reference.media_type
                })
            });
            if !allowed {
                return Err("configuration schema is outside the selected closure".into());
            }
            contracts::check_artifact_bytes(
                &reference,
                bytes
                    .get(&reference.digest)
                    .ok_or("configuration schema bytes unavailable")?,
            )
            .map_err(|e| e.to_string())?;
            let schemas = manifests
                .values()
                .flat_map(|m| &m.files)
                .filter(|f| f.media_type == "application/schema+json")
                .map(|f| {
                    Ok((
                        f.digest.clone(),
                        bytes.get(&f.digest).ok_or("schema unavailable")?.clone(),
                    ))
                })
                .collect::<Result<BTreeMap<_, _>>>()?;
            contracts::prepare_closure(&schemas).map_err(|e| e.to_string())?;
        }
        let now = reader.current_time(initial);
        catalog.fresh(
            record
                .snapshot
                .as_ref()
                .ok_or("snapshot")?
                .release
                .created_at,
            now,
            record.policy.max_age,
            record.policy.skew,
        )?;
        checkpoint_current(
            record,
            &record.id,
            &record.snapshot.as_ref().ok_or("snapshot")?.release.id,
            now,
        )?;
        for (id, event) in &events {
            let body = nostr::ext::parse_record(event).map_err(|e| e.to_string())?;
            let owner = Id::parse(catalog::text(&body, "package")?)?;
            checkpoint_current(record, &owner, id, now)?;
        }
        self.retain(reader)?;
        let snapshot = record.snapshot.as_ref().ok_or("registry snapshot")?;
        let root = events.get(&root_id).ok_or("root release")?.clone();
        let selection=Selection{v:"openagents.registry-selection.v1".into(),registry:record.id.clone(),catalog_release:snapshot.release.id.clone(),catalog_manifest:parse_artifact(&nostr::ext::parse_record(&snapshot.release).map_err(|e|e.to_string())?["manifest"]).map_err(|e|e.to_string())?.digest,catalog_digest:snapshot.catalog_digest.clone(),catalog_revision:snapshot.revision,package:package.clone(),release:root,dependencies:events,manifest:raw_root.ok_or("selected manifest")?,selected_at:now,sources:record.sources.clone(),usage:reader.usage()};
        let key = digest_bytes(format!("{}:{}", record.id.as_str(), package.as_str()).as_bytes());
        atomic(
            &self.root.join(format!("selections/{}.json", &key[7..])),
            &serde_json::to_vec(&selection).map_err(|e| e.to_string())?,
        )?;
        Ok(selection)
    }
    /// Local history is inert. Reuse still checks remembered revocations and
    /// the operator's explicit offline/current-membership policy.
    pub fn prior_selection(&self, id: &Id, package: &Id, now: u64) -> Result<Selection> {
        let record = self.record(id)?;
        let key = digest_bytes(format!("{}:{}", id.as_str(), package.as_str()).as_bytes());
        let value = parse_strict_bounded(
            &read(
                &self.root.join(format!("selections/{}.json", &key[7..])),
                RECORD_LIMIT,
            )?,
            RECORD_LIMIT,
        )
        .map_err(|e| e.to_string())?;
        let selection: Selection =
            serde_json::from_value(value).map_err(|_| "selection record is unreadable")?;
        if selection.v != "openagents.registry-selection.v1"
            || &selection.registry != id
            || &selection.package != package
        {
            return Err("selection record has another identity".into());
        }
        let current = self.browse(id, now, false).is_ok()
            && self
                .cached_catalog(&record)?
                .entry(package)
                .is_some_and(|entry| entry["release"]["id"] == selection.release.id);
        if (record.policy.require_current_membership || !record.policy.allow_pinned_use) && !current
        {
            return Err("current registry membership is required to reuse this pin".into());
        }
        if selection
            .dependencies
            .get(&selection.release.id)
            .is_none_or(|e| e != &selection.release)
        {
            return Err("selection omits its root release".into());
        }
        if nostr::ext::parse_record(&selection.release).map_err(|e| e.to_string())?["package"]
            != package.as_str()
        {
            return Err("selected release belongs to another package".into());
        }
        let mut manifests = BTreeMap::new();
        let mut files = BTreeMap::new();
        for (id, event) in &selection.dependencies {
            event.validate_crypto().map_err(|e| e.to_string())?;
            if event.id != *id || event.kind != 3184 {
                return Err("selection dependency identity differs".into());
            }
            let body = nostr::ext::parse_record(event).map_err(|e| e.to_string())?;
            let owner = Id::parse(catalog::text(&body, "package")?)?;
            let reference = parse_artifact(&body["manifest"]).map_err(|e| e.to_string())?;
            let value = contracts::parse_strict(&self.cached_bytes(&reference)?)
                .map_err(|e| e.to_string())?;
            let manifest = nostr::ext::parse_manifest(&value).map_err(|e| e.to_string())?;
            if manifest.package != owner.as_str()
                || body["version"] != manifest.version
                || id == &selection.release.id && value != selection.manifest
            {
                return Err("cached selection differs from its publisher manifest".into());
            }
            for file in &manifest.files {
                let reference = ArtifactRef {
                    digest: file.digest.clone(),
                    size: file.size,
                    media_type: file.media_type.clone(),
                    schema: None,
                    event: None,
                    sources: Vec::new(),
                };
                files.insert(file.digest.clone(), self.cached_bytes(&reference)?);
            }
            manifests.insert(id.clone(), manifest);
            if record.policy.require_current_membership || !record.policy.allow_pinned_use {
                checkpoint_current(&record, &owner, id, now)?;
            }
            if let Some(state) = record.revocations.get(owner.as_str()) {
                for revoked in state.events.values() {
                    let body = nostr::ext::parse_record(revoked).map_err(|e| e.to_string())?;
                    if body["release"]["id"] == *id {
                        return Err("publisher revoked this selected dependency".into());
                    }
                }
            }
        }
        for (id, manifest) in &manifests {
            nostr::ext::verify_closure(&nostr::ext::Closure {
                manifest,
                files: &files,
                staged: &manifest
                    .files
                    .iter()
                    .map(|f| f.path.clone())
                    .collect::<Vec<_>>(),
                dependencies: &manifests,
                root: id,
                byte_limit: record.policy.limits.bytes,
            })
            .map_err(|e| e.to_string())?;
        }
        selection
            .release
            .validate_crypto()
            .map_err(|e| e.to_string())?;
        Ok(selection)
    }
}
fn checkpoint_current(record: &Record, package: &Id, release: &str, now: u64) -> Result<()> {
    let state = record
        .revocations
        .get(package.as_str())
        .ok_or("publisher revocation state is unavailable")?;
    let accepted = state
        .accepted
        .as_ref()
        .ok_or("publisher checkpoint is unavailable")?;
    if state.observed.as_ref().is_none_or(|e| e.id != accepted.id) {
        return Err("a newer publisher checkpoint is not yet verified".into());
    }
    let body = nostr::ext::parse_record(accepted).map_err(|e| e.to_string())?;
    if accepted.kind != 30185
        || accepted.pubkey != package.publisher()
        || body["package"] != package.as_str()
    {
        return Err("publisher checkpoint has another package identity".into());
    }
    if now.saturating_sub(catalog::number(&body, "as_of")?) > record.policy.max_age {
        return Err("publisher checkpoint exceeds its maximum age".into());
    }
    nostr::ext::fresh(&nostr::ext::Freshness {
        as_of: catalog::number(&body, "as_of")?,
        valid_until: catalog::number(&body, "valid_until")?,
        now,
        skew: record.policy.skew,
        present: true,
        empty_answer: false,
        strict: true,
        explicit_pin: false,
    })
    .map_err(|e| e.to_string())?;
    for event in state.events.values() {
        let body = nostr::ext::parse_record(event).map_err(|e| e.to_string())?;
        if body["release"]["id"] == release {
            return Err("publisher revoked this release".into());
        }
    }
    Ok(())
}
