//! Bounded, operator-selected discovery of exact signed publications.
//!
//! This reader verifies public evidence. It has no installation, account,
//! credential, execution, or payment authority. Curation and local reviews
//! remain attributed operator decisions; publisher and evaluator signatures
//! are checked separately. This is not a general NIP-REG reader.

use std::collections::{BTreeMap, BTreeSet};

use nostr::contracts::{
    ArtifactRef, check_artifact_bytes, digest_bytes, parse_artifact, parse_strict,
};
use nostr::domain::Event;
use nostr::{cap, eval_ext, ext};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

pub const SCHEMA: &str = "openagents.discovery.curated.v1";
pub const SNAPSHOT_SCHEMA: &str = "openagents.discovery.snapshot.v1";
pub const MAX_CATALOG_BYTES: usize = 256 * 1024;
pub const MAX_RECORD_BYTES: usize = 256 * 1024;
pub const MAX_ARTIFACT_BYTES: usize = 16 * 1024 * 1024;
pub const MAX_ITEMS: usize = 64;
pub const MAX_EVENTS: usize = 512;

/// An explicit local curation admission. Display names never select identity.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Catalog {
    pub schema: String,
    pub curator: String,
    pub max_age_seconds: u64,
    pub skew_seconds: u64,
    pub items: Vec<Item>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Item {
    /// Exact publisher-qualified package or service namespace.
    pub id: String,
    /// `extension` or `service`.
    pub kind: String,
    /// Exact release or capability-head event ID.
    pub event: String,
    /// Extension component slug, or the service door name.
    pub operation: String,
    /// Exact extension manifest digest, or CAP definition digest.
    pub digest: String,
    #[serde(default)]
    pub evaluations: Vec<String>,
    pub review: Option<Review>,
    /// Optional advisory observation. It cannot change verified readiness.
    pub reputation: Option<Value>,
}

/// An attributed local review of exact evidence, not remote attestation.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Review {
    pub reviewer: String,
    pub reviewed_at: u64,
    pub valid_until: u64,
    pub event: String,
    pub digest: String,
    pub operation: String,
    pub evaluations: Vec<String>,
    /// Explicit signed release fee; endpoint and fulfillment prices are separate.
    pub publisher_fee_msat: Option<u64>,
    pub data_requirements: Vec<String>,
    pub recipients: Vec<String>,
    pub limitations: Vec<String>,
}

/// A caller supplies only bounded public objects from admitted sources.
pub trait Source {
    fn heads(&mut self, kind: u16, publisher: &str, slug: &str) -> Result<Vec<Event>, String>;
    fn event(&mut self, id: &str) -> Result<Event, String>;
    fn artifact(&mut self, reference: &ArtifactRef) -> Result<Vec<u8>, String>;
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Snapshot {
    pub schema: String,
    pub catalog_digest: String,
    pub curator: String,
    pub observed_at: u64,
    pub cards: Vec<Card>,
    /// Retain these complete signed records with the snapshot for later refresh.
    pub evidence: Vec<Event>,
    pub limitations: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Card {
    pub id: String,
    pub kind: String,
    pub selected_event: String,
    pub selected_digest: String,
    pub selected_operation: String,
    pub title: String,
    pub description: String,
    pub state: String,
    pub error: Option<String>,
    pub publication: Value,
    pub operation: Value,
    pub price: Value,
    pub evaluation: Vec<Value>,
    pub review: Value,
    pub readiness: Value,
    pub reputation: Value,
    pub search_relevance: u64,
    pub purchase_authorized: bool,
    /// Verified bytes for the native adapter; never emitted as listing prose.
    #[serde(skip)]
    pub component: Option<Value>,
}

fn hex(text: &str) -> bool {
    text.len() == 64
        && text
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

fn identity(id: &str) -> Result<(&str, &str), String> {
    let (key, slug) = id
        .split_once(':')
        .ok_or("Use an exact publisher-qualified identity.")?;
    if !hex(key)
        || slug.is_empty()
        || slug.len() > 128
        || !slug
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"-_./".contains(&b))
    {
        return Err("Invalid publisher-qualified identity.".into());
    }
    Ok((key, slug))
}

fn display(text: &str, limit: usize) -> String {
    text.chars()
        .filter(|c| !c.is_control())
        .take(limit)
        .collect()
}

/// Validate the admission before reading any source or resolving a selection.
pub fn parse_catalog(bytes: &[u8]) -> Result<Catalog, String> {
    if bytes.len() > MAX_CATALOG_BYTES {
        return Err("Curated catalog exceeds 256 KiB.".into());
    }
    let value = parse_strict(bytes).map_err(|e| e.to_string())?;
    let catalog: Catalog =
        serde_json::from_value(value).map_err(|_| "Invalid curated catalog.".to_owned())?;
    if catalog.schema != SCHEMA
        || catalog.curator.is_empty()
        || catalog.curator.len() > 128
        || catalog.curator.chars().any(char::is_control)
        || catalog.max_age_seconds == 0
        || catalog.skew_seconds > 300
        || catalog.items.is_empty()
        || catalog.items.len() > MAX_ITEMS
    {
        return Err("Unsupported or unbounded curated catalog.".into());
    }
    let mut selected = BTreeSet::new();
    for item in &catalog.items {
        identity(&item.id)?;
        let namespace = item.id.split_once(':').unwrap().1;
        if item.kind == "extension" && namespace.contains('/')
            || item.kind == "service"
                && (namespace.matches('/').count() != 1 || namespace.split('/').any(str::is_empty))
        {
            return Err("Use a package identity for an extension and a qualified component identity for a service.".into());
        }
        if !matches!(item.kind.as_str(), "extension" | "service")
            || !hex(&item.event)
            || !item.digest.strip_prefix("sha256:").is_some_and(hex)
            || item.operation.is_empty()
            || item.operation.len() > 128
            || item.operation.chars().any(char::is_control)
            || item.evaluations.len() > 32
            || item.evaluations.iter().any(|id| !hex(id))
            || item.evaluations.iter().collect::<BTreeSet<_>>().len() != item.evaluations.len()
            || !selected.insert((&item.id, &item.operation))
            || item
                .reputation
                .as_ref()
                .is_some_and(|v| v.to_string().len() > 4096)
        {
            return Err("Invalid or duplicated curated selection.".into());
        }
        if let Some(review) = &item.review {
            if review.reviewer.is_empty()
                || review.reviewer.len() > 128
                || review.reviewer.chars().any(char::is_control)
                || review.valid_until <= review.reviewed_at
                || [
                    &review.data_requirements,
                    &review.recipients,
                    &review.limitations,
                ]
                .iter()
                .any(|items| {
                    items.len() > 32
                        || items.iter().any(|s| {
                            s.is_empty() || s.len() > 1024 || s.chars().any(char::is_control)
                        })
                })
            {
                return Err("Invalid local qualification review.".into());
            }
        }
    }
    Ok(catalog)
}

struct Reader<'a> {
    source: &'a mut dyn Source,
    known: BTreeMap<String, Event>,
    evidence: BTreeMap<String, Event>,
    artifacts: BTreeMap<String, Vec<u8>>,
    bytes: usize,
    event_bytes: usize,
    lookups: usize,
    now: u64,
    age: u64,
    skew: u64,
}

impl Reader<'_> {
    fn keep(&mut self, event: Event) -> Result<Event, String> {
        let size = serde_json::to_vec(&event)
            .map_err(|_| "Invalid event.")?
            .len();
        if size > MAX_RECORD_BYTES
            || self.evidence.len() >= MAX_EVENTS && !self.evidence.contains_key(&event.id)
        {
            return Err("Signed discovery evidence exceeds its bounds.".into());
        }
        if !self.evidence.contains_key(&event.id) {
            self.event_bytes = self
                .event_bytes
                .checked_add(size)
                .ok_or("Discovery evidence byte count overflow.")?;
            if self.event_bytes > 4 * 1024 * 1024 {
                return Err("Signed discovery evidence exceeds 4 MiB.".into());
            }
        }
        event
            .validate_nip01_structure()
            .and_then(|()| event.validate_crypto())
            .map_err(|_| "Invalid signed discovery event.".to_owned())?;
        if event.created_at > self.now.saturating_add(self.skew) || event.is_expired(self.now) {
            return Err("Signed discovery event is future-dated or expired.".into());
        }
        self.evidence.insert(event.id.clone(), event.clone());
        Ok(event)
    }

    fn event(&mut self, id: &str) -> Result<Event, String> {
        if let Some(event) = self.evidence.get(id) {
            return self.keep(event.clone());
        }
        self.lookup()?;
        let event = self.source.event(id)?;
        if event.id != id {
            return Err("Source substituted an event identity.".into());
        }
        self.keep(event)
    }

    fn head(&mut self, kind: u16, publisher: &str, slug: &str) -> Result<Event, String> {
        self.lookup()?;
        let events = self.source.heads(kind, publisher, slug)?;
        if events.is_empty() || events.len() > 64 {
            return Err("Current signed discovery head is unavailable or unbounded.".into());
        }
        // A newer signed malformed body must refuse; it cannot revive an older head.
        let mut checked = Vec::new();
        for event in events {
            let event = self.keep(event)?;
            if event.kind != kind
                || event.pubkey != publisher
                || event.tag_values("d").collect::<Vec<_>>() != [slug]
            {
                return Err("Source substituted a discovery namespace.".into());
            }
            checked.push(event);
        }
        let head = checked
            .into_iter()
            .max_by(|a, b| {
                a.created_at
                    .cmp(&b.created_at)
                    .then_with(|| b.id.cmp(&a.id))
            })
            .unwrap();
        if self.now.saturating_sub(head.created_at) > self.age {
            return Err("Signed discovery head is stale.".into());
        }
        for known in self.known.values().filter(|e| {
            e.kind == kind && e.pubkey == publisher && e.tag_values("d").any(|d| d == slug)
        }) {
            if known.created_at > head.created_at
                || known.created_at == head.created_at && known.id < head.id
            {
                return Err("Discovery head rolls back retained signed evidence.".into());
            }
        }
        Ok(head)
    }

    fn artifact(&mut self, reference: &ArtifactRef) -> Result<Vec<u8>, String> {
        if reference.size > MAX_ARTIFACT_BYTES as u64 {
            return Err("Discovery artifact exceeds 16 MiB.".into());
        }
        let bytes = if let Some(bytes) = self.artifacts.get(&reference.digest) {
            bytes.clone()
        } else {
            self.lookup()?;
            let bytes = self.source.artifact(reference)?;
            self.bytes = self
                .bytes
                .checked_add(bytes.len())
                .ok_or("Discovery byte count overflow.")?;
            if self.bytes > MAX_ARTIFACT_BYTES {
                return Err("Discovery sources exceed 16 MiB.".into());
            }
            self.artifacts
                .insert(reference.digest.clone(), bytes.clone());
            bytes
        };
        check_artifact_bytes(reference, &bytes).map_err(|e| e.to_string())?;
        Ok(bytes)
    }

    fn lookup(&mut self) -> Result<(), String> {
        self.lookups += 1;
        if self.lookups > 1024 {
            return Err("Discovery source lookups exceed 1,024 objects.".into());
        }
        Ok(())
    }

    fn checkpoint(
        &mut self,
        id: &str,
        publisher: &str,
        slug: &str,
        release: &str,
    ) -> Result<Value, String> {
        let event = self.head(ext::CHECKPOINT_KIND, publisher, slug)?;
        let body = ext::parse_record(&event).map_err(|e| e.to_string())?;
        if body["package"] != id {
            return Err("Checkpoint names another package.".into());
        }
        ext::fresh(&ext::Freshness {
            as_of: body["as_of"].as_u64().unwrap_or(0),
            valid_until: body["valid_until"].as_u64().unwrap_or(0),
            now: self.now,
            skew: self.skew,
            present: true,
            empty_answer: false,
            strict: true,
            explicit_pin: true,
        })
        .map_err(|e| e.to_string())?;
        if self.now.saturating_sub(body["as_of"].as_u64().unwrap_or(0)) > self.age {
            return Err("Publisher checkpoint observation is stale.".into());
        }
        let ids = checkpoint_ids(&body)?;
        let mut knowledge = ext::RevocationKnowledge {
            revision: 0,
            revocations: BTreeSet::new(),
        };
        let mut previous: Vec<_> = self
            .known
            .values()
            .filter(|e| {
                e.kind == ext::CHECKPOINT_KIND
                    && e.pubkey == publisher
                    && e.tag_values("d").any(|d| d == slug)
            })
            .cloned()
            .collect();
        previous.sort_by_key(|e| e.created_at);
        for prior in previous {
            let prior_body = ext::parse_record(&prior).map_err(|e| e.to_string())?;
            let prior_ids = checkpoint_ids(&prior_body)?;
            knowledge = ext::absorb(
                &knowledge,
                prior_body["revision"].as_u64().unwrap_or(0),
                &prior_ids,
            )
            .map_err(|e| e.to_string())?;
        }
        // Revision zero is not a usable initial checkpoint under this profile.
        let revision = body["revision"]
            .as_u64()
            .filter(|r| *r > 0)
            .ok_or("Checkpoint needs a positive revision.")?;
        if knowledge.revision == 0 {
            knowledge.revision = revision;
            knowledge.revocations = ids.iter().cloned().collect();
        } else {
            knowledge = ext::absorb(&knowledge, revision, &ids).map_err(|e| e.to_string())?;
        }
        for revocation in self
            .known
            .values()
            .filter(|e| e.kind == ext::REVOCATION_KIND && e.pubkey == publisher)
        {
            let revoked = ext::parse_record(revocation).map_err(|e| e.to_string())?;
            if revoked["package"] == id && revoked["release"]["id"] == release {
                return Err("Publisher revoked this exact release.".into());
            }
        }
        for revocation_id in &knowledge.revocations {
            let revocation = self.event(revocation_id)?;
            let revoked = ext::parse_record(&revocation).map_err(|e| e.to_string())?;
            if revocation.kind != ext::REVOCATION_KIND
                || revocation.pubkey != publisher
                || revoked["package"] != id
            {
                return Err(
                    "Checkpoint revocation belongs to another publisher or package.".into(),
                );
            }
            if revoked["release"]["id"] == release {
                return Err("Publisher revoked this exact release.".into());
            }
        }
        Ok(
            json!({"event": event.id, "revision": revision, "as_of": body["as_of"], "valid_until": body["valid_until"], "revocations": ids}),
        )
    }
}

fn checkpoint_ids(body: &Value) -> Result<Vec<String>, String> {
    let ids = body["revocations"]
        .as_array()
        .ok_or("Artifact-backed checkpoints are unavailable in this profile.")?;
    if ids.len() > 128 {
        return Err("Checkpoint exceeds 128 revocations.".into());
    }
    ids.iter()
        .map(|id| {
            id.as_str()
                .filter(|id| hex(id))
                .map(str::to_owned)
                .ok_or("Invalid checkpoint revocation ID.".to_owned())
        })
        .collect()
}

fn check_evaluation_lock(
    value: &Value,
    subject: &nostr::contracts::DefinitionRef,
    operation: &str,
    component_digest: &str,
) -> Result<(), String> {
    if value["v"] == "openagents.ext-eval-lock.v1" {
        let definition =
            nostr::contracts::parse_definition(&value["definition"]).map_err(|e| e.to_string())?;
        // The owning ext-eval runner pins package.json as the subject, then
        // retains each program's exact bytes separately in this run lock.
        if value["requires"] != json!([])
            || value["arm"] != "subject"
            || definition.id != subject.id
            || definition.artifact != subject.artifact
            || value["programs"].as_array().is_none_or(|programs| {
                let selected: Vec<_> = programs.iter().filter(|p| p["slug"] == operation).collect();
                selected.len() != 1 || selected[0]["digest"] != component_digest
            })
        {
            return Err(
                "Evaluation run lock does not retain the exact package and selected program."
                    .into(),
            );
        }
    } else {
        let lock = nostr::contracts::parse_lock(value).map_err(|e| e.to_string())?;
        if lock.root != subject.id
            || lock
                .entries
                .iter()
                .find(|e| e.id == subject.id)
                .is_none_or(|e| e.definition != *subject)
        {
            return Err("Evaluation lock does not retain the exact selected subject.".into());
        }
    }
    Ok(())
}

fn check_evaluation_suite(
    reader: &mut Reader<'_>,
    publication: &eval_ext::Publication,
) -> Result<Value, String> {
    let event = reader.event(&publication.suite_release.id)?;
    if event.kind != ext::RELEASE_KIND || event.pubkey != publication.suite_release.pubkey {
        return Err("Evaluation suite release signer or kind differs from its report.".into());
    }
    let body = ext::parse_record(&event).map_err(|e| e.to_string())?;
    let manifest_ref = parse_artifact(&body["manifest"]).map_err(|e| e.to_string())?;
    if manifest_ref.schema.as_deref() != Some("openagents.package.v1") {
        return Err("Evaluation suite manifest has an unsupported schema.".into());
    }
    let manifest_bytes = reader.artifact(&manifest_ref)?;
    if manifest_bytes.len() > MAX_RECORD_BYTES {
        return Err("Evaluation suite manifest exceeds 256 KiB.".into());
    }
    let release = eval_ext::parse_release(&event, &manifest_bytes).map_err(|e| e.to_string())?;
    if body["version"] != release.manifest.version {
        return Err("Evaluation suite version differs from its signed release.".into());
    }
    if release.manifest.files.len() > 256 || !release.manifest.dependencies.is_empty() {
        return Err("Evaluation suite closure is unsupported or unbounded.".into());
    }
    let suite_bytes = reader.artifact(&publication.report.suite)?;
    let suite = eval_ext::parse_suite(&suite_bytes).map_err(|e| e.to_string())?;
    if suite.acceptance.artifact.digest != publication.report.profile.gate {
        return Err("Evaluation report gate differs from its published suite.".into());
    }
    let manifest_value = parse_strict(&manifest_bytes).map_err(|e| e.to_string())?;
    let cases = reader.artifact(&suite.cases)?;
    let package = eval_ext::check_suite_package(&manifest_value, &suite_bytes, &cases)
        .map_err(|e| e.to_string())?;
    if suite.id != format!("{}/{}", release.package, package.slug) {
        return Err("Evaluation suite identity differs from its signed publisher package.".into());
    }
    let mut files = BTreeMap::new();
    for file in &release.manifest.files {
        let reference = ArtifactRef {
            digest: file.digest.clone(),
            size: file.size,
            media_type: file.media_type.clone(),
            schema: None,
            event: None,
            sources: vec![],
        };
        files.insert(file.digest.clone(), reader.artifact(&reference)?);
    }
    let dependencies = BTreeMap::from([(event.id.clone(), release.manifest.clone())]);
    ext::verify_closure(&ext::Closure {
        manifest: &release.manifest,
        files: &files,
        staged: &[],
        dependencies: &dependencies,
        root: &event.id,
        byte_limit: MAX_ARTIFACT_BYTES as u64,
    })
    .map_err(|e| e.to_string())?;
    if package
        .cases
        .cases
        .iter()
        .map(|c| (&c.id, c.kind))
        .collect::<Vec<_>>()
        != publication
            .report
            .profile
            .cases
            .iter()
            .map(|(id, kind)| (id, *kind))
            .collect::<Vec<_>>()
    {
        return Err("Evaluation report cases differ from the published suite scope.".into());
    }
    Ok(
        json!({"event":event.id,"id":suite.id,"workload":suite.workload.digest,"cases":suite.cases.digest,"gate":suite.acceptance.artifact.digest}),
    )
}

/// Refresh every admitted identity. Individual refusals remain visible cards.
/// Prior signed evidence preserves head watermarks and publisher revocations.
pub fn discover(
    bytes: &[u8],
    source: &mut dyn Source,
    previous: &[Event],
    now: u64,
) -> Result<Snapshot, String> {
    let catalog = parse_catalog(bytes)?;
    if previous.len() > MAX_EVENTS {
        return Err("Retained discovery evidence exceeds 512 events.".into());
    }
    let mut reader = Reader {
        source,
        known: BTreeMap::new(),
        evidence: BTreeMap::new(),
        artifacts: BTreeMap::new(),
        bytes: 0,
        event_bytes: 0,
        lookups: 0,
        now,
        age: catalog.max_age_seconds,
        skew: catalog.skew_seconds,
    };
    for event in previous {
        // Historical expiration does not clear a valid head watermark or revocation.
        event
            .validate_nip01_structure()
            .and_then(|()| event.validate_crypto())
            .map_err(|_| "Invalid retained discovery signature.")?;
        if serde_json::to_vec(event)
            .map_err(|_| "Invalid retained event.")?
            .len()
            > MAX_RECORD_BYTES
            || event.created_at > now.saturating_add(reader.skew)
        {
            return Err("Invalid retained discovery evidence bounds.".into());
        }
        reader.known.insert(event.id.clone(), event.clone());
        reader.event_bytes = reader
            .event_bytes
            .checked_add(
                serde_json::to_vec(event)
                    .map_err(|_| "Invalid retained event.")?
                    .len(),
            )
            .ok_or("Retained evidence byte count overflow.")?;
        if reader.event_bytes > 4 * 1024 * 1024 {
            return Err("Retained discovery evidence exceeds 4 MiB.".into());
        }
        reader.evidence.insert(event.id.clone(), event.clone());
    }
    let mut cards = Vec::new();
    for item in &catalog.items {
        let mut card = Card {
            id: item.id.clone(),
            kind: item.kind.clone(),
            selected_event: item.event.clone(),
            selected_digest: item.digest.clone(),
            selected_operation: item.operation.clone(),
            title: item.id.clone(),
            description: String::new(),
            state: "unavailable".into(),
            error: None,
            publication: Value::Null,
            operation: Value::Null,
            price: json!({"state":"unknown", "total_price":"requires_separate_current_quote"}),
            evaluation: Vec::new(),
            review: json!({"state":"unknown"}),
            readiness: json!({"state":"unqualified","purchase_authorized":false,"reason":"Native support, current measured review, and the owning purchase path remain separate."}),
            reputation: json!({"advisory_only":true,"observation":item.reputation}),
            search_relevance: 0,
            purchase_authorized: false,
            component: None,
        };
        match resolve(&mut reader, item, &mut card) {
            Ok(()) => card.state = "verified_discovery".into(),
            Err(error) => {
                card.error = Some(error);
            }
        }
        cards.push(card);
    }
    Ok(Snapshot { schema: SNAPSHOT_SCHEMA.into(), catalog_digest: digest_bytes(bytes), curator: catalog.curator, observed_at: now, cards, evidence: reader.evidence.into_values().collect(), limitations: vec!["Curation and qualification reviews are explicit local operator decisions, not publisher or evaluator signatures.".into(), "Signed publications and evaluation reports are attributable claims; this reader does not probe availability or rerun measurements.".into(), "The source can withhold newer records. Fresh signed evidence and retained watermarks do not prove global completeness.".into(), "Selection grants no installation, disclosure, execution, capacity, or spending authority. The owning client must obtain and approve a separate current quote.".into()] })
}

fn resolve(reader: &mut Reader<'_>, item: &Item, card: &mut Card) -> Result<(), String> {
    let (publisher, slug) = identity(&item.id)?;
    let (definition_id, definition_digest, component_digest) = if item.kind == "extension" {
        let listing = reader.head(ext::LISTING_KIND, publisher, slug)?;
        let listing_body = ext::parse_record(&listing).map_err(|e| e.to_string())?;
        card.title = display(listing_body["title"].as_str().unwrap_or(&item.id), 128);
        card.description = display(
            listing_body["description"].as_str().unwrap_or_default(),
            2048,
        );
        if listing_body["state"] != "published" {
            card.state = "withdrawn".into();
            return Err("Publisher withdrew this listing.".into());
        }
        if listing_body["package"] != item.id || listing_body["release"]["id"] != item.event {
            return Err("Current listing names a different exact release.".into());
        }
        if listing_body["release"]["pubkey"] != publisher
            || listing_body["release"]["kind"] != ext::RELEASE_KIND
        {
            return Err("Listing release reference names another publisher or event kind.".into());
        }
        let release = reader.event(&item.event)?;
        let body = ext::parse_record(&release).map_err(|e| e.to_string())?;
        if release.kind != ext::RELEASE_KIND
            || release.pubkey != publisher
            || body["package"] != item.id
        {
            return Err("Release signer or package differs from the admitted identity.".into());
        }
        let checkpoint = reader.checkpoint(&item.id, publisher, slug, &item.event)?;
        let reference = parse_artifact(&body["manifest"]).map_err(|e| e.to_string())?;
        if reference.digest != item.digest
            || reference.schema.as_deref() != Some("openagents.package.v1")
        {
            return Err("Release manifest differs from the admitted digest or schema.".into());
        }
        let manifest_bytes = reader.artifact(&reference)?;
        if manifest_bytes.len() > MAX_RECORD_BYTES {
            return Err("Discovery manifest exceeds 256 KiB.".into());
        }
        let value = parse_strict(&manifest_bytes).map_err(|e| e.to_string())?;
        let manifest = ext::parse_manifest(&value).map_err(|e| e.to_string())?;
        if manifest.package != item.id || body["version"] != manifest.version {
            return Err("Manifest identity differs from the signed release.".into());
        }
        if !manifest.dependencies.is_empty() {
            return Err("Dependency closure is unavailable in this discovery profile.".into());
        }
        if manifest.files.len() > 256 {
            return Err("Discovery closure exceeds 256 files.".into());
        }
        let mut files = BTreeMap::new();
        for file in &manifest.files {
            let reference = ArtifactRef {
                digest: file.digest.clone(),
                size: file.size,
                media_type: file.media_type.clone(),
                schema: None,
                event: None,
                sources: vec![],
            };
            files.insert(file.digest.clone(), reader.artifact(&reference)?);
        }
        let dependencies = BTreeMap::from([(release.id.clone(), manifest.clone())]);
        ext::verify_closure(&ext::Closure {
            manifest: &manifest,
            files: &files,
            staged: &[],
            dependencies: &dependencies,
            root: &release.id,
            byte_limit: MAX_ARTIFACT_BYTES as u64,
        })
        .map_err(|e| e.to_string())?;
        let component = value["components"]
            .as_array()
            .and_then(|components| components.iter().find(|c| c["slug"] == item.operation))
            .ok_or("Selected operation is absent from the exact manifest.")?;
        let definition = parse_artifact(&component["definition"]).map_err(|e| e.to_string())?;
        let bytes = files
            .get(&definition.digest)
            .ok_or("Selected component bytes are unavailable.")?;
        check_artifact_bytes(&definition, bytes).map_err(|e| e.to_string())?;
        let record = parse_strict(bytes).map_err(|e| e.to_string())?;
        let id = format!("{}/{}", item.id, item.operation);
        match component["kind"].as_str() {
            Some("program") => {
                let parsed = nostr::prg::parse_definition(&record["definition"])
                    .map_err(|e| e.to_string())?;
                if parsed.id != id {
                    return Err("Program definition names a different exact component.".into());
                }
            }
            Some("capability") => {
                let parsed =
                    cap::parse_definition(&record["definition"]).map_err(|e| e.to_string())?;
                if parsed.id != id {
                    return Err("Capability definition names a different exact component.".into());
                }
            }
            _ => return Err("This component is not a supported operation description.".into()),
        }
        card.component = Some(record);
        let provenance = if value["provenance"].to_string().len() <= 4096 {
            value["provenance"].clone()
        } else {
            json!({"state":"bounded_summary","source":display(value["provenance"]["source"].as_str().unwrap_or("unknown"),512),"full_manifest":reference.digest})
        };
        card.publication = json!({"listing":listing.id,"release":release.id,"publisher":publisher,"version":body["version"],"manifest":reference.digest,"provenance":provenance,"checkpoint":checkpoint,"closure_verified":true});
        card.operation = json!({"id":id,"kind":component["kind"],"definition":definition.digest,"declared":true,"availability":"not_probed","host_support":"requires_native_inspection"});
        card.price = json!({"state":if body.get("fee_msat").is_some(){"signed_publisher_fee"}else{"unknown"},"publisher_fee_msat":body.get("fee_msat"),"payout":body.get("payout"),"source_release":release.id,"total_price":"requires_separate_current_quote"});
        let measured_digest = if component["kind"] == "program" {
            let package_file = manifest
                .files
                .iter()
                .find(|f| f.path == "package.json")
                .ok_or("Published program has no exact package record for evaluation linkage.")?;
            let package_record =
                parse_strict(&files[&package_file.digest]).map_err(|e| e.to_string())?;
            // The existing native publisher uses package.json as the program
            // descriptor. Inspect that exact owner shape, not an unrelated
            // descriptor selected by display wording.
            let descriptor = parse_artifact(&component["descriptor"]).map_err(|e| e.to_string())?;
            if descriptor.digest != package_file.digest
                || descriptor.schema.as_deref() != Some("openagents.coder-package.v1")
            {
                return Err(
                    "Native program descriptor differs from its exact package record.".into(),
                );
            }
            check_artifact_bytes(&descriptor, &files[&package_file.digest])
                .map_err(|e| e.to_string())?;
            if package_record["publisher"] != publisher
                || package_record["slug"] != slug
                || package_record["version"] != manifest.version
            {
                return Err(
                    "Native package record differs from the signed publisher or release.".into(),
                );
            }
            let text = std::str::from_utf8(bytes).map_err(|_| "Native program is not UTF-8.")?;
            let pin = digest_bytes(
                &serde_json::to_vec(text).map_err(|_| "Cannot digest native program.")?,
            );
            if package_record["program"]["name"] != item.operation
                || package_record["program"]["digest"] != pin.trim_start_matches("sha256:")
            {
                return Err(
                    "Native package program reference differs from the signed component bytes."
                        .into(),
                );
            }
            package_file.digest.clone()
        } else {
            definition.digest.clone()
        };
        (id, measured_digest, definition.digest)
    } else {
        let head_slug = slug
            .rsplit_once('/')
            .ok_or("Service identity must name its exact capability component.")?
            .1;
        let event = reader.head(cap::DISCOVERY_KIND, publisher, head_slug)?;
        if event.id != item.event {
            return Err("Current service head differs from the admitted event.".into());
        }
        let manifest = cap::resolve_service(
            [&event],
            &cap::ServiceTrust {
                publisher,
                slug: head_slug,
                max_age_seconds: reader.age,
            },
            reader.now,
        )
        .map_err(|e| e.to_string())?;
        let body = parse_strict(event.content.as_bytes()).map_err(|e| e.to_string())?;
        let digest =
            digest_bytes(&nostr::contracts::jcs(&body["definition"]).map_err(|e| e.to_string())?);
        if digest != item.digest || manifest.definition.id != item.id {
            return Err("Service definition differs from the admitted identity or digest.".into());
        }
        let door = manifest
            .contract
            .door(&item.operation)
            .ok_or("Selected service door is not advertised.")?;
        card.description = display(&manifest.definition.summary, 2048);
        card.publication = json!({"event":event.id,"publisher":publisher,"created_at":event.created_at,"provenance":"signed_service_publication","max_age_seconds":reader.age});
        card.operation = json!({"id":manifest.definition.id,"door":door.name,"model":door.model,"artifact_signature":door.artifact_signature,"interface":manifest.contract.interface,"transports":manifest.contract.lanes.iter().map(|lane| &lane.transport).collect::<Vec<_>>(),"availability":"not_probed","host_support":"requires_native_inspection"});
        card.component = Some(body);
        (manifest.definition.id, digest.clone(), digest)
    };
    let mut has_pass = false;
    for id in &item.evaluations {
        let checked = (|| -> Result<Value, String> {
            let event = reader.event(id)?;
            let publication = eval_ext::parse_publication(&event).map_err(|e| e.to_string())?;
            let subject = &publication.report.subject;
            if publication.subject_release.as_ref().is_none_or(|pointer| {
                pointer.id != item.event
                    || pointer.pubkey != publisher
                    || pointer.kind
                        != if item.kind == "extension" {
                            ext::RELEASE_KIND
                        } else {
                            cap::DISCOVERY_KIND
                        }
            }) || subject.definition.id != definition_id
                || subject.definition.artifact.digest != definition_digest
            {
                return Err(
                    "Evaluation subject differs from the selected event or component digest."
                        .into(),
                );
            }
            let lock_bytes = reader.artifact(&subject.lock)?;
            let lock_value = parse_strict(&lock_bytes).map_err(|e| e.to_string())?;
            check_evaluation_lock(
                &lock_value,
                &subject.definition,
                &item.operation,
                &component_digest,
            )?;
            reader.artifact(&subject.configuration)?;
            let suite = check_evaluation_suite(reader, &publication)?;
            if publication.report.ended_at > reader.now.saturating_add(reader.skew)
                || reader.now.saturating_sub(publication.report.ended_at) > reader.age
            {
                return Err("Evaluation measurement is stale or future-dated.".into());
            }
            has_pass |= publication.verdict() == eval_ext::Verdict::Pass;
            Ok(
                json!({"state":"signed_measurement","event":event.id,"evaluator":publication.evaluator,"verdict":publication.verdict().word(),"distribution":publication.distribution(),"subject":definition_id,"lock":subject.lock.digest,"configuration":subject.configuration.digest,"started_at":publication.report.started_at,"ended_at":publication.report.ended_at,"cases":publication.report.profile.cases.len(),"gate":publication.report.profile.gate,"suite":suite,"independently_rerun_here":false,"limitations":"Published scope and attributable evaluator claims only; no general reliability, delivery, or payment authority."}),
            )
        })();
        card.evaluation.push(match checked {
            Ok(value) => value,
            Err(error) => json!({"state":"unverified","event":id,"error":error}),
        });
    }
    if let Some(review) = &item.review {
        let exact = review.event == item.event
            && review.digest == item.digest
            && review.operation == item.operation
            && review.evaluations == item.evaluations;
        let current = review.reviewed_at <= reader.now
            && reader.now < review.valid_until
            && reader.now.saturating_sub(review.reviewed_at) <= reader.age;
        let price = item.kind == "extension"
            && card.price["publisher_fee_msat"]
                .as_u64()
                .is_some_and(|fee| Some(fee) == review.publisher_fee_msat);
        card.review = json!({"state":if exact && current && price && has_pass && card.evaluation.iter().all(|e| e["state"] == "signed_measurement" && e["verdict"] == "pass"){"current_scoped_review"}else{"unqualified"},"reviewer":review.reviewer,"source":"local_operator_admission","exact_pins":exact,"current":current,"signed_price_matches":price,"has_scoped_passing_measurement":has_pass,"data_requirements":review.data_requirements,"recipients":review.recipients,"limitations":review.limitations,"valid_until":review.valid_until});
    }
    Ok(())
}

/// Search ranks signed names and exact identities only, independently of scores.
pub fn search(snapshot: &mut Snapshot, query: &str) {
    let words: Vec<_> = query.split_whitespace().map(str::to_lowercase).collect();
    for card in &mut snapshot.cards {
        let text = format!(
            "{} {} {} {}",
            card.id, card.title, card.description, card.operation
        )
        .to_lowercase();
        card.search_relevance = words
            .iter()
            .filter(|word| text.contains(word.as_str()))
            .count() as u64;
        if !query.is_empty() && card.id == query {
            card.search_relevance = u64::MAX;
        }
    }
    snapshot.cards.sort_by(|a, b| {
        b.search_relevance
            .cmp(&a.search_relevance)
            .then_with(|| a.id.cmp(&b.id))
            .then_with(|| a.selected_event.cmp(&b.selected_event))
    });
}
